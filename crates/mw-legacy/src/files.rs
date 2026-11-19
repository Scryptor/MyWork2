//! Копии файлов старой папки: сначала в промежуточную папку, на место — только после успешной записи в базу.

use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use mw_core::ImportedLegacyRow;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;

use crate::LegacyError;
use crate::convert::PlannedFile;
use crate::folder::LegacyFolder;
use crate::progress::{ImportStage, Progress, StepProgress};
use crate::report::{FileStats, WarningCode, Warnings};

/// Папки внутри `attachments`, которые занимает импорт.
pub(crate) const IMPORT_DIRS: [&str; 3] = ["orders", "stock", "legacy"];

/// Начало имени промежуточной папки: `attachments/.import-<время>`.
const STAGING_PREFIX: &str = ".import-";

/// Отметка промежуточной папки (`StagingMark`): ставится перед записью базы.
const MARK: &str = ".import-mark.json";

/// Ход и отмена копирования.
#[derive(Clone, Default)]
pub(crate) struct Control {
    pub(crate) progress: Option<Progress>,
    pub(crate) cancel: Option<CancellationToken>,
}

impl Control {
    pub(crate) fn cancelled(&self) -> bool {
        self.cancel
            .as_ref()
            .is_some_and(CancellationToken::is_cancelled)
    }
}

/// Отметка промежуточной папки. Без номера импорта — предварительная (перед записью базы): записана ли
/// база, решает имя папки в отчёте последнего импорта (`staging`). С номером — база записана.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct StagingMark {
    /// Папки из `IMPORT_DIRS`, которые лежали в промежуточной папке перед установкой.
    staged: Vec<String>,
    /// Папка копии `backups/<время>` при замене: прежние файлы уходят в её `attachments`.
    backup: Option<PathBuf>,
    import_id: Option<i64>,
}

/// Промежуточная папка `attachments/.import-<время>` со скопированными файлами.
#[derive(Debug)]
pub(crate) struct Staging {
    dir: PathBuf,
    /// Папку убирает `Drop`, пока не вызваны `abort`, `mark_written` или `commit`.
    armed: bool,
    mark: Option<StagingMark>,
}

fn io_error(context: String) -> impl FnOnce(std::io::Error) -> LegacyError {
    move |source| LegacyError::Io { context, source }
}

/// Папка не пуста (или не читается: тогда считаем занятой).
pub(crate) fn dir_has_entries(dir: &Path) -> bool {
    match std::fs::read_dir(dir) {
        Ok(mut entries) => entries.next().is_some(),
        Err(e) => e.kind() != std::io::ErrorKind::NotFound,
    }
}

/// Путь внутри `attachments`: только обычные сегменты.
fn is_plain_relative(path: &Path) -> bool {
    path.components().count() > 0 && path.components().all(|c| matches!(c, Component::Normal(_)))
}

/// Свободное имя в наборе: при совпадении к имени файла добавляется « (2)», « (3)».
fn unique_dest(dest: &Path, used: &mut BTreeSet<String>) -> PathBuf {
    let key = |p: &Path| p.to_string_lossy().to_lowercase();
    if used.insert(key(dest)) {
        return dest.to_path_buf();
    }
    let stem = dest
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let extension = dest
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let parent = dest.parent().map(Path::to_path_buf).unwrap_or_default();
    for n in 2.. {
        let candidate = parent.join(format!("{stem} ({n}){extension}"));
        if used.insert(key(&candidate)) {
            return candidate;
        }
    }
    unreachable!("бесконечный перебор суффиксов")
}

enum Copied {
    Done { size: u64, sha256: String },
    Unreadable(std::io::Error),
}

/// Копирует файл, считая размер и sha256. Ошибка чтения источника — не фатальна, записи — фатальна.
fn copy_hashing(from: &Path, to: &Path) -> Result<Copied, LegacyError> {
    let mut input = match std::fs::File::open(from) {
        Ok(f) => f,
        Err(e) => return Ok(Copied::Unreadable(e)),
    };
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(io_error(format!(
            "Не удалось создать папку {}",
            parent.display()
        )))?;
    }
    let mut output = std::fs::File::create(to).map_err(io_error(format!(
        "Не удалось создать файл {}",
        to.display()
    )))?;
    let mut hasher = Sha256::new();
    let mut size = 0u64;
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let n = match input.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                drop(output);
                std::fs::remove_file(to).ok();
                return Ok(Copied::Unreadable(e));
            }
        };
        hasher.update(&buffer[..n]);
        output.write_all(&buffer[..n]).map_err(io_error(format!(
            "Не удалось записать файл {}",
            to.display()
        )))?;
        size += n as u64;
    }
    output.flush().map_err(io_error(format!(
        "Не удалось записать файл {}",
        to.display()
    )))?;
    Ok(Copied::Done {
        size,
        sha256: hex::encode(hasher.finalize()),
    })
}

/// Где лежит файл плана в старой папке; `None` — не найден или место назначения не внутри `attachments`.
fn locate(folder: &LegacyFolder, file: &PlannedFile) -> Option<PathBuf> {
    let found = file
        .source
        .to_str()
        .and_then(|relative| folder.find_file(relative))?;
    is_plain_relative(&file.dest).then_some(found)
}

/// Сколько файлов плана найдено и их общий размер в байтах (только чтение).
pub(crate) fn measure(folder: &LegacyFolder, files: &[PlannedFile]) -> (u64, u64) {
    files
        .iter()
        .filter_map(|file| locate(folder, file))
        .filter_map(|path| std::fs::metadata(path).ok())
        .fold((0, 0), |(count, bytes), m| (count + 1, bytes + m.len()))
}

/// Копирует файлы в `attachments_dir/.import-<stamp>`; возвращает следы для базы и счётчики.
/// Ход — этап `Copying` по числу файлов; между файлами проверяется отмена (тогда папка удаляется).
///
/// Источник каждый раз заново ищется через `LegacyFolder::find_file`: путь не выходит за пределы старой папки.
pub(crate) fn stage(
    folder: &LegacyFolder,
    files: &[PlannedFile],
    attachments_dir: &Path,
    stamp: &str,
    warnings: &mut Warnings,
    control: &Control,
) -> Result<(Staging, Vec<ImportedLegacyRow>, FileStats), LegacyError> {
    std::fs::create_dir_all(attachments_dir).map_err(io_error(format!(
        "Не удалось создать папку {}",
        attachments_dir.display()
    )))?;
    let mut dir = attachments_dir.join(format!("{STAGING_PREFIX}{stamp}"));
    let mut n = 1;
    while dir.exists() {
        n += 1;
        dir = attachments_dir.join(format!("{STAGING_PREFIX}{stamp}-{n}"));
    }
    std::fs::create_dir(&dir).map_err(io_error(format!(
        "Не удалось создать папку {}",
        dir.display()
    )))?;
    let staging = Staging {
        dir,
        armed: true,
        mark: None,
    };
    let mut step = StepProgress::new(control.progress.as_ref(), ImportStage::Copying, files.len());
    match copy_all(folder, files, &staging, warnings, control, &mut step) {
        Ok((rows, stats)) => Ok((staging, rows, stats)),
        Err(e) => {
            staging.abort();
            Err(e)
        }
    }
}

fn copy_all(
    folder: &LegacyFolder,
    files: &[PlannedFile],
    staging: &Staging,
    warnings: &mut Warnings,
    control: &Control,
    step: &mut StepProgress,
) -> Result<(Vec<ImportedLegacyRow>, FileStats), LegacyError> {
    let mut rows = Vec::new();
    let mut stats = FileStats::default();
    let mut used = BTreeSet::new();
    for file in files {
        #[cfg(feature = "testing")]
        crate::testing::pause(&folder.root, ImportStage::Copying);
        if control.cancelled() {
            return Err(LegacyError::Cancelled);
        }
        step.step();
        let shown = file.source.display().to_string();
        let Some(from) = locate(folder, file) else {
            warnings.push(
                WarningCode::FileMissing,
                "file",
                &file.legacy_key,
                file.legacy_source,
                &shown,
                "Файл не найден в старой папке и не перенесён.",
            );
            stats.missing += 1;
            continue;
        };
        let dest = unique_dest(&file.dest, &mut used);
        match copy_hashing(&from, &staging.dir.join(&dest))? {
            Copied::Done { size, sha256 } => {
                stats.copied += 1;
                stats.bytes += size;
                let stored_as =
                    format!("attachments/{}", dest.to_string_lossy().replace('\\', "/"));
                rows.push(ImportedLegacyRow {
                    source: file.legacy_source.to_owned(),
                    source_key: file.legacy_key.clone(),
                    target: file.target.clone(),
                    raw: json!({ "stored_as": stored_as, "size": size, "sha256": sha256 }),
                });
            }
            Copied::Unreadable(e) => {
                stats.missing += 1;
                warnings.push(
                    WarningCode::FileUnreadable,
                    "file",
                    &file.legacy_key,
                    file.legacy_source,
                    &shown,
                    &format!("Файл не прочитан и не перенесён: {e}."),
                );
            }
        }
    }
    Ok((rows, stats))
}

/// Переносит папку; между дисками — копированием. `Some(текст)` — перенос удался, но исходную папку убрать не вышло.
fn move_dir(from: &Path, to: &Path) -> Result<Option<String>, LegacyError> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(None);
    }
    copy_dir(from, to)?;
    Ok(std::fs::remove_dir_all(from).err().map(|e| {
        format!(
            "Папка {} перенесена, но не удалена на старом месте: {e}.",
            from.display()
        )
    }))
}

fn copy_dir(from: &Path, to: &Path) -> Result<(), LegacyError> {
    std::fs::create_dir_all(to).map_err(io_error(format!(
        "Не удалось создать папку {}",
        to.display()
    )))?;
    let entries = std::fs::read_dir(from).map_err(io_error(format!(
        "Не удалось прочитать папку {}",
        from.display()
    )))?;
    for entry in entries {
        let entry = entry.map_err(io_error(format!(
            "Не удалось прочитать папку {}",
            from.display()
        )))?;
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target).map_err(io_error(format!(
                "Не удалось скопировать файл {}",
                entry.path().display()
            )))?;
        }
    }
    Ok(())
}

impl Drop for Staging {
    fn drop(&mut self) {
        if self.armed
            && let Err(e) = std::fs::remove_dir_all(&self.dir)
        {
            tracing::debug!(error = %e, "промежуточная папка не удалена");
        }
    }
}

fn cleanup_warning(warnings: &mut Warnings, path: &Path, message: &str) {
    warnings.push(
        WarningCode::FileUnreadable,
        "file",
        &path.display().to_string(),
        "",
        "",
        message,
    );
}

impl Staging {
    pub(crate) fn path(&self) -> &Path {
        &self.dir
    }

    /// Имя папки (`.import-<время>`): пишется в отчёт импорта, по нему запуск сервера узнаёт свою папку.
    pub(crate) fn name(&self) -> String {
        self.dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// Удаляет промежуточную папку со всем содержимым.
    pub(crate) fn abort(mut self) {
        self.armed = false;
        std::fs::remove_dir_all(&self.dir).ok();
    }

    /// Предварительная отметка перед записью базы: какие папки лежат здесь и куда при замене уходят прежние.
    pub(crate) fn mark_pending(&mut self, backup: Option<&Path>) -> Result<(), LegacyError> {
        let mark = StagingMark {
            staged: present_dirs(&self.dir),
            backup: backup.map(Path::to_path_buf),
            import_id: None,
        };
        write_mark(&self.dir, &mark)?;
        self.mark = Some(mark);
        Ok(())
    }

    /// База записана: папку больше не удаляет `Drop`, в отметку пишется номер импорта.
    pub(crate) fn mark_written(&mut self, import_id: i64) -> Result<(), LegacyError> {
        self.armed = false;
        let mut mark = self.mark.clone().unwrap_or_else(|| StagingMark {
            staged: present_dirs(&self.dir),
            backup: None,
            import_id: None,
        });
        mark.import_id = Some(import_id);
        write_mark(&self.dir, &mark)?;
        self.mark = Some(mark);
        Ok(())
    }

    /// Ставит файлы на место. С `previous` (папка копии) прежние `orders`, `stock` и `legacy` уходят туда.
    /// Ошибка — файлы не поставлены (остаются в промежуточной папке); неудачная уборка — только предупреждение.
    pub(crate) fn commit(
        mut self,
        attachments_dir: &Path,
        previous: Option<&Path>,
        warnings: &mut Warnings,
    ) -> Result<(), LegacyError> {
        // База уже записана: промежуточную папку после этого не удаляем, пока файлы не на месте.
        self.armed = false;
        let staged = match &self.mark {
            Some(mark) => mark.staged.clone(),
            None => present_dirs(&self.dir),
        };
        place(&self.dir, attachments_dir, previous, &staged, warnings)
    }
}

/// Какие из `IMPORT_DIRS` лежат в папке.
fn present_dirs(dir: &Path) -> Vec<String> {
    IMPORT_DIRS
        .iter()
        .filter(|name| dir.join(name).is_dir())
        .map(|name| (*name).to_owned())
        .collect()
}

/// Пишет отметку через временный файл: оборванная запись не оставляет испорченную отметку.
fn write_mark(dir: &Path, mark: &StagingMark) -> Result<(), LegacyError> {
    let path = dir.join(MARK);
    let temporary = dir.join(format!("{MARK}.tmp"));
    let context = || format!("Не удалось записать отметку {}", path.display());
    let text = serde_json::to_vec(mark).map_err(|e| LegacyError::Io {
        context: context(),
        source: std::io::Error::other(e),
    })?;
    std::fs::write(&temporary, text).map_err(io_error(context()))?;
    std::fs::rename(&temporary, &path).map_err(io_error(context()))
}

/// Отметка папки: `Ok(None)` — отметки нет, `Err` — есть, но не читается.
fn read_mark(dir: &Path) -> Result<Option<StagingMark>, String> {
    match std::fs::read(dir.join(MARK)) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| e.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Нет ничего или пустая папка: сюда можно поставить папку из промежуточной.
fn missing_or_empty(path: &Path) -> bool {
    match std::fs::symlink_metadata(path) {
        Err(e) => e.kind() == std::io::ErrorKind::NotFound,
        Ok(m) if m.is_dir() => std::fs::read_dir(path).is_ok_and(|mut e| e.next().is_none()),
        Ok(_) => false,
    }
}

/// Ставит папки `staged` из промежуточной папки `dir` на место; годится и для продолжения оборванной
/// установки. С `previous` (замена) прежняя папка уходит туда, пока на её место ещё ничего не поставлено:
/// её новой копии ещё нет в `dir` (или её не было вовсе), а в `previous` такой папки ещё нет.
/// Папка ставится, только если на её месте ничего нет или пусто; иначе — ошибка, `dir` остаётся.
fn place(
    dir: &Path,
    attachments_dir: &Path,
    previous: Option<&Path>,
    staged: &[String],
    warnings: &mut Warnings,
) -> Result<(), LegacyError> {
    if let Some(previous) = previous {
        for name in IMPORT_DIRS {
            let old = attachments_dir.join(name);
            let not_placed = dir.join(name).exists() || !staged.iter().any(|s| s == name);
            if not_placed && old.exists() && !previous.join(name).exists() {
                std::fs::create_dir_all(previous).map_err(io_error(format!(
                    "Не удалось создать папку {}",
                    previous.display()
                )))?;
                if let Some(note) = move_dir(&old, &previous.join(name))? {
                    cleanup_warning(warnings, &old, &note);
                }
            }
        }
    }
    for name in IMPORT_DIRS {
        let from = dir.join(name);
        if !from.exists() {
            continue;
        }
        let target = attachments_dir.join(name);
        if !missing_or_empty(&target) {
            return Err(LegacyError::AttachmentsNotEmpty(target));
        }
        // Пустую папку можно заменить.
        if target.is_dir() {
            std::fs::remove_dir(&target).ok();
        }
        if let Some(note) = move_dir(&from, &target)? {
            cleanup_warning(warnings, &from, &note);
        }
    }
    if let Err(e) = std::fs::remove_dir_all(dir) {
        cleanup_warning(
            warnings,
            dir,
            &format!("Промежуточная папка не удалена: {e}."),
        );
    }
    Ok(())
}

/// Последний импорт в базе: номер и имя его промежуточной папки (`staging` в отчёте).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LatestImport {
    pub(crate) id: i64,
    pub(crate) staging: Option<String>,
}

/// Итог уборки промежуточных папок при запуске.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct StagingCleanup {
    /// Удалены: импорт оборвался до записи базы или его уже заменил более новый.
    pub removed: Vec<PathBuf>,
    /// Файлы последнего импорта поставлены на место, папка удалена.
    pub placed: Vec<PathBuf>,
    /// Оставлены: место занято, файлы не переносятся, отметка не читается или папку не удалить.
    pub kept: Vec<PathBuf>,
}

/// Разбирает промежуточные папки `attachments/.import-*`, оставшиеся после оборванного импорта.
/// Без отметки — удаляется (обрыв до записи базы). С отметкой: если это папка последнего импорта
/// в базе (`latest`), установка доводится до конца; если нет — база её не знает (не записана или
/// заменена более новым импортом), папка удаляется.
pub(crate) fn finish_staging(
    attachments_dir: &Path,
    latest: Option<&LatestImport>,
) -> StagingCleanup {
    let mut cleanup = StagingCleanup::default();
    let Ok(entries) = std::fs::read_dir(attachments_dir) else {
        return cleanup;
    };
    let mut found: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with(STAGING_PREFIX))
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .collect();
    found.sort();
    for dir in found {
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mark = match read_mark(&dir) {
            Ok(mark) => mark,
            Err(e) => {
                tracing::warn!(error = %e, path = %dir.display(), "отметка промежуточной папки не читается");
                cleanup.kept.push(dir);
                continue;
            }
        };
        let current = mark.as_ref().filter(|m| {
            latest.is_some_and(|l| {
                l.staging.as_deref() == Some(name.as_str())
                    && m.import_id.is_none_or(|id| id == l.id)
            })
        });
        let Some(mark) = current else {
            match std::fs::remove_dir_all(&dir) {
                Ok(()) => cleanup.removed.push(dir),
                Err(e) => {
                    tracing::warn!(error = %e, path = %dir.display(), "промежуточная папка не удалена");
                    cleanup.kept.push(dir);
                }
            }
            continue;
        };
        let previous = mark.backup.as_ref().map(|b| b.join("attachments"));
        let mut notes = Warnings::default();
        match place(
            &dir,
            attachments_dir,
            previous.as_deref(),
            &mark.staged,
            &mut notes,
        ) {
            Ok(()) => cleanup.placed.push(dir),
            Err(e) => {
                tracing::warn!(error = %e, path = %dir.display(), "файлы прерванного импорта не поставлены на место");
                cleanup.kept.push(dir);
            }
        }
        for note in notes.list() {
            tracing::warn!("{}", note.message);
        }
    }
    cleanup
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::FixtureVersion::V32;
    use crate::folder::scan_folder;
    use crate::test_support::fixture;

    fn planned(source: &str, dest: &str) -> PlannedFile {
        PlannedFile {
            source: PathBuf::from(source),
            dest: PathBuf::from(dest),
            legacy_source: "ClientFiles",
            legacy_key: source.to_owned(),
            target: None,
        }
    }

    #[tokio::test]
    async fn staging_is_removed_on_abort() {
        let src = fixture(V32).await;
        let folder = scan_folder(src.path()).unwrap();
        let attachments = tempfile::tempdir().unwrap();
        let mut w = Warnings::default();
        let files = [
            planned("ClientFiles/1/1/photo.jpg", "orders/1/photo.jpg"),
            planned("ClientFiles/2/3/video.mp4", "orders/3/video.mp4"),
        ];
        let (staging, rows, stats) = stage(
            &folder,
            &files,
            attachments.path(),
            "20261111-090000",
            &mut w,
            &Control::default(),
        )
        .unwrap();
        assert_eq!((rows.len(), stats.copied, stats.missing), (2, 2, 0));
        assert!(staging.dir.join("orders/1/photo.jpg").exists());
        staging.abort();
        assert_eq!(std::fs::read_dir(attachments.path()).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn clashes_get_a_suffix_and_missing_files_are_reported() {
        let src = fixture(V32).await;
        let folder = scan_folder(src.path()).unwrap();
        let attachments = tempfile::tempdir().unwrap();
        let mut w = Warnings::default();
        let files = [
            planned("ClientFiles/1/1/photo.jpg", "orders/1/a.jpg"),
            planned("ClientFiles/1/1/docs/act.pdf", "orders/1/A.jpg"),
            planned("ClientFiles/nope.txt", "orders/1/n.txt"),
            planned("../outside", "orders/1/o.txt"),
        ];
        let (staging, rows, stats) = stage(
            &folder,
            &files,
            attachments.path(),
            "x",
            &mut w,
            &Control::default(),
        )
        .unwrap();
        assert_eq!((stats.copied, stats.missing), (2, 2));
        assert_eq!(rows[1].raw["stored_as"], "attachments/orders/1/A (2).jpg");
        assert_eq!(w.list().len(), 2);
        staging.abort();
    }

    #[tokio::test]
    async fn commit_replaces_empty_folder_and_reports_conflicts() {
        let src = fixture(V32).await;
        let folder = scan_folder(src.path()).unwrap();
        let files = [planned("ClientFiles/1/1/photo.jpg", "orders/1/photo.jpg")];

        let attachments = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(attachments.path().join("orders")).unwrap();
        let mut w = Warnings::default();
        let (staging, _, _) = stage(
            &folder,
            &files,
            attachments.path(),
            "a",
            &mut w,
            &Control::default(),
        )
        .unwrap();
        staging.commit(attachments.path(), None, &mut w).unwrap();
        assert!(attachments.path().join("orders/1/photo.jpg").exists());
        assert!(w.list().is_empty());
        assert!(!attachments.path().join(".import-a").exists());

        let attachments = tempfile::tempdir().unwrap();
        let (staging, _, _) = stage(
            &folder,
            &files,
            attachments.path(),
            "b",
            &mut w,
            &Control::default(),
        )
        .unwrap();
        std::fs::create_dir_all(attachments.path().join("orders/keep")).unwrap();
        std::fs::write(attachments.path().join("orders/keep/f"), b"x").unwrap();
        let err = staging
            .commit(attachments.path(), None, &mut w)
            .unwrap_err();
        assert!(matches!(err, LegacyError::AttachmentsNotEmpty(_)));
        // Файлы остались в промежуточной папке.
        assert!(
            attachments
                .path()
                .join(".import-b/orders/1/photo.jpg")
                .exists()
        );
    }

    /// Папка `.import-<name>` с файлом в `orders` и (если задана) отметкой.
    fn staged_dir(att: &Path, name: &str, mark: Option<&StagingMark>) -> PathBuf {
        let dir = att.join(format!(".import-{name}"));
        std::fs::create_dir_all(dir.join("orders/1")).unwrap();
        std::fs::write(dir.join("orders/1/new.jpg"), name).unwrap();
        if let Some(mark) = mark {
            write_mark(&dir, mark).unwrap();
        }
        dir
    }

    fn mark(import_id: Option<i64>) -> StagingMark {
        StagingMark {
            staged: vec!["orders".into()],
            backup: None,
            import_id,
        }
    }

    #[test]
    fn start_places_only_the_latest_import_and_removes_the_rest() {
        let att = tempfile::tempdir().unwrap();
        let unmarked = staged_dir(att.path(), "a", None);
        let pending_lost = staged_dir(att.path(), "b", Some(&mark(None)));
        let superseded = staged_dir(att.path(), "c", Some(&mark(Some(4))));
        let broken = att.path().join(".import-d");
        std::fs::create_dir_all(&broken).unwrap();
        std::fs::write(broken.join(MARK), b"{").unwrap();
        // База записана, а отметка с номером — нет: решает имя папки в отчёте импорта.
        let pending_written = staged_dir(att.path(), "e", Some(&mark(None)));
        let latest = LatestImport {
            id: 5,
            staging: Some(".import-e".into()),
        };
        let cleanup = finish_staging(att.path(), Some(&latest));
        assert_eq!(cleanup.removed, [unmarked, pending_lost, superseded]);
        assert_eq!(cleanup.kept, [broken]);
        assert_eq!(cleanup.placed, std::slice::from_ref(&pending_written));
        assert_eq!(
            std::fs::read(att.path().join("orders/1/new.jpg")).unwrap(),
            b"e"
        );
        assert!(!pending_written.exists());

        // Тот же номер, но папка другого импорта (номера в SQLite могут повториться после замены).
        let att = tempfile::tempdir().unwrap();
        let other = staged_dir(att.path(), "f", Some(&mark(Some(5))));
        let cleanup = finish_staging(att.path(), Some(&latest));
        assert_eq!(cleanup.removed, [other]);
        let cleanup = finish_staging(att.path(), None);
        assert_eq!(cleanup, StagingCleanup::default());
    }

    #[test]
    fn resumed_replace_moves_old_files_to_the_backup_only_once() {
        let att = tempfile::tempdir().unwrap();
        let previous = att.path().join("backup/attachments");
        // Прежние `orders` и `legacy` на месте, новые `orders` и `stock` ещё в промежуточной папке.
        for (dir, file) in [("orders/1", "old.jpg"), ("legacy/ClientFiles", "old.txt")] {
            std::fs::create_dir_all(att.path().join(dir)).unwrap();
            std::fs::write(att.path().join(dir).join(file), b"old").unwrap();
        }
        let staging = att.path().join(".import-x");
        for dir in ["orders/1", "stock/1"] {
            std::fs::create_dir_all(staging.join(dir)).unwrap();
            std::fs::write(staging.join(dir).join("new.jpg"), b"new").unwrap();
        }
        let staged = vec!["orders".to_owned(), "stock".to_owned()];
        let mut w = Warnings::default();
        place(&staging, att.path(), Some(&previous), &staged, &mut w).unwrap();
        assert!(previous.join("orders/1/old.jpg").exists());
        assert!(previous.join("legacy/ClientFiles/old.txt").exists());
        assert!(
            att.path().join("orders/1/new.jpg").exists()
                && att.path().join("stock/1/new.jpg").exists()
        );
        assert!(!att.path().join("legacy").exists() && !staging.exists());

        // Обрыв после установки `orders`: при продолжении новые `orders` в копию не уходят.
        let att = tempfile::tempdir().unwrap();
        let previous = att.path().join("backup/attachments");
        std::fs::create_dir_all(previous.join("orders/1")).unwrap();
        std::fs::create_dir_all(att.path().join("orders/1")).unwrap();
        std::fs::write(att.path().join("orders/1/new.jpg"), b"new").unwrap();
        let staging = att.path().join(".import-y");
        std::fs::create_dir_all(staging.join("stock/1")).unwrap();
        std::fs::write(staging.join("stock/1/new.jpg"), b"new").unwrap();
        place(&staging, att.path(), Some(&previous), &staged, &mut w).unwrap();
        assert!(att.path().join("orders/1/new.jpg").exists());
        assert!(!previous.join("orders/1/new.jpg").exists());
        assert!(att.path().join("stock/1/new.jpg").exists() && !staging.exists());
    }

    #[tokio::test]
    async fn dropping_staging_removes_the_folder() {
        let src = fixture(V32).await;
        let folder = scan_folder(src.path()).unwrap();
        let attachments = tempfile::tempdir().unwrap();
        let mut w = Warnings::default();
        let files = [planned("ClientFiles/1/1/photo.jpg", "orders/1/photo.jpg")];
        let (staging, _, _) = stage(
            &folder,
            &files,
            attachments.path(),
            "c",
            &mut w,
            &Control::default(),
        )
        .unwrap();
        drop(staging);
        assert_eq!(std::fs::read_dir(attachments.path()).unwrap().count(), 0);
    }
}
