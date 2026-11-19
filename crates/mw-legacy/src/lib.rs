//! Чтение копии базы и папки MyWork 2 и перенос их в MyWork 3.

mod check;
mod convert;
mod error;
mod files;
mod folder;
mod parse;
mod progress;
mod report;
mod report_files;
mod source;
mod text;
mod view;

#[cfg(feature = "fixtures")]
pub mod fixture;

#[cfg(feature = "testing")]
pub mod testing;

#[cfg(test)]
mod test_support;

use std::path::{Path, PathBuf};

use mw_core::{Actor, Core, DatabaseKind, ImportMode};
use tokio_util::sync::CancellationToken;

pub use check::{FolderCheck, check_folder, check_target};
pub use error::{LegacyError, StartedFrom, import_error_message};
pub use files::StagingCleanup;
pub use progress::{ImportJob, ImportStage, Progress};
pub use report::{FileStats, ImportReport, Severity, Warning, WarningCode};
pub use report_files::{REPORTS_DIR, write_report_files};
pub use source::SchemaSummary;
pub use view::{ImportReportView, LegacyImportItem, ReportGroup, import_item, report_view};

/// Папка копий перед заменой в папке данных (`<папка данных>/backups`); при замене в ней появляется
/// `<время>/`. Одна для импорта командой и из интерфейса и для проверки папки, которая её называет.
pub const BACKUPS_DIR: &str = "backups";

/// `<папка данных>/backups` — см. `BACKUPS_DIR`.
pub fn backups_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(BACKUPS_DIR)
}

/// Что и куда импортировать.
#[derive(Clone)]
pub struct ImportOptions {
    /// Папка MyWork 2 (только чтение).
    pub source: PathBuf,
    /// Папка вложений MyWork 3.
    pub attachments_dir: PathBuf,
    /// Папка копий; при замене в ней появляется `<время>/`.
    pub backups_dir: PathBuf,
    /// Заменить уже имеющиеся данные (только SQLite), предварительно сняв копию.
    pub replace: bool,
    /// Ход по этапам; `Done` приходит последним и только при успехе.
    pub progress: Option<Progress>,
    /// Отмена (остановка программы): проверяется между этапами и файлами до начала записи в базу;
    /// с начала записи импорт доводится до конца.
    pub cancel: Option<CancellationToken>,
}

impl std::fmt::Debug for ImportOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImportOptions")
            .field("source", &self.source)
            .field("attachments_dir", &self.attachments_dir)
            .field("backups_dir", &self.backups_dir)
            .field("replace", &self.replace)
            .field("progress", &self.progress.is_some())
            .field("cancel", &self.cancel.is_some())
            .finish()
    }
}

impl ImportOptions {
    fn emit(&self, stage: ImportStage, percent: u8) {
        if let Some(progress) = &self.progress {
            progress(stage, percent);
        }
    }

    fn control(&self) -> files::Control {
        files::Control {
            progress: self.progress.clone(),
            cancel: self.cancel.clone(),
        }
    }

    /// Отмена до начала записи: ошибка `Cancelled`.
    fn check_cancelled(&self) -> Result<(), LegacyError> {
        if self.control().cancelled() {
            return Err(LegacyError::Cancelled);
        }
        Ok(())
    }
}

/// Ошибка `spawn_blocking`: задача файлового этапа не дошла до конца.
fn interrupted(context: &str) -> impl FnOnce(tokio::task::JoinError) -> LegacyError {
    move |e| LegacyError::Io {
        context: context.to_owned(),
        source: std::io::Error::other(e),
    }
}

/// Свободная папка `backups_dir/<время>` (при совпадении времени — с суффиксом).
fn backup_folder(backups_dir: &Path, stamp: &str) -> PathBuf {
    let mut dir = backups_dir.join(stamp);
    let mut n = 1;
    while dir.exists() {
        n += 1;
        dir = backups_dir.join(format!("{stamp}-{n}"));
    }
    dir
}

/// Лежит ли `dir` внутри `root`: путь может ещё не существовать — тогда берётся ближайшая существующая папка выше,
/// а `.` и `..` в несуществующем хвосте убираются по тексту. Если решить нельзя, считается, что лежит внутри
/// (лучше отказать, чем записать в старую папку).
fn lies_inside(dir: &Path, root: &Path) -> bool {
    use std::path::Component;
    let Ok(root) = root.canonicalize() else {
        return true;
    };
    let Ok(absolute) = std::path::absolute(dir) else {
        return true;
    };
    let components: Vec<Component> = absolute.components().collect();
    // Самый длинный существующий префикс.
    let mut existing = components.len();
    let canonical = loop {
        let prefix: PathBuf = components[..existing].iter().collect();
        if let Ok(canonical) = prefix.canonicalize() {
            break canonical;
        }
        if existing <= 1 {
            return true;
        }
        existing -= 1;
    };
    let mut resolved = canonical;
    for component in &components[existing..] {
        match component {
            Component::Normal(name) => resolved.push(name),
            Component::ParentDir => {
                if !resolved.pop() {
                    return true;
                }
            }
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) => return true,
        }
    }
    resolved.starts_with(&root)
}

/// Лежит ли `path` внутри старой папки; `source` — папка MyWork 2 или файл `baza.sqlite` в ней.
pub fn lies_inside_source(path: &Path, source: &Path) -> bool {
    let root = if source.is_file() {
        source
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
    } else {
        source
    };
    lies_inside(path, root)
}

/// Импортирует папку MyWork 2: читает, преобразует, копирует файлы через промежуточную папку,
/// пишет пакет одной транзакцией и только после успеха ставит файлы на место.
/// Файловые операции идут в `spawn_blocking`; ход — через `options.progress`.
pub async fn import_folder(
    core: &Core,
    actor: &Actor,
    options: &ImportOptions,
) -> Result<ImportReport, LegacyError> {
    options.emit(ImportStage::Reading, 0);
    core.check_import_rights(actor)?;
    let source = options.source.clone();
    let folder = tokio::task::spawn_blocking(move || folder::scan_folder(&source))
        .await
        .map_err(interrupted("Чтение папки прервано"))??;
    for dir in [&options.attachments_dir, &options.backups_dir] {
        if lies_inside(dir, &folder.root) {
            return Err(LegacyError::InsideSource(dir.clone()));
        }
    }
    let db = source::read_database(&folder.db_file).await?;
    tracing::info!(tables = db.tables.len(), "импорт: старая база прочитана");
    options.emit(ImportStage::Reading, 100);
    options.check_cancelled()?;

    if options.replace && core.database_kind() != DatabaseKind::Sqlite {
        return Err(LegacyError::ReplaceUnsupported);
    }
    if !options.replace {
        let target = core.import_target_state().await?;
        if !target.is_empty() {
            return Err(LegacyError::TargetNotEmpty(target));
        }
        if let Some(dir) = files::occupied_import_dir(&options.attachments_dir) {
            return Err(LegacyError::AttachmentsNotEmpty(dir));
        }
    }

    let started_at = core.now();
    let stamp = started_at.format("%Y%m%d-%H%M%S").to_string();
    options.emit(ImportStage::Converting, 0);
    let mut conversion = tokio::task::spawn_blocking({
        let folder = folder.clone();
        move || convert::convert(&db, &folder, started_at)
    })
    .await
    .map_err(interrupted("Разбор данных прерван"))?;
    options.emit(ImportStage::Converting, 100);
    options.check_cancelled()?;
    tracing::info!(
        orders = conversion.bundle.orders.len(),
        clients = conversion.bundle.clients.len(),
        history = conversion.bundle.history.len(),
        files = conversion.files.len(),
        "импорт: данные преобразованы"
    );

    let attachments_dir = options.attachments_dir.clone();
    let stage_stamp = stamp.clone();
    let (mut staging, rows, file_stats, warnings) = tokio::task::spawn_blocking({
        let folder = folder.clone();
        let planned = std::mem::take(&mut conversion.files);
        let mut warnings = std::mem::take(&mut conversion.warnings);
        let control = options.control();
        move || {
            let staged = files::stage(
                &folder,
                &planned,
                &attachments_dir,
                &stage_stamp,
                &mut warnings,
                &control,
            );
            staged.map(|(s, r, f)| (s, r, f, warnings))
        }
    })
    .await
    .map_err(interrupted("Копирование файлов прервано"))??;
    options.emit(ImportStage::Copying, 100);
    tracing::info!(
        copied = file_stats.copied,
        missing = file_stats.missing,
        bytes = file_stats.bytes,
        "импорт: файлы скопированы в промежуточную папку"
    );

    if let Err(e) = options.check_cancelled() {
        staging.abort();
        tracing::info!("импорт прерван до записи в базу, промежуточная папка удалена");
        return Err(e);
    }
    let backup = options
        .replace
        .then(|| backup_folder(&options.backups_dir, &stamp));
    // Без предварительной отметки запуск сервера удалил бы папку, даже если база успеет записаться.
    if let Err(e) = staging.mark_pending(&options.attachments_dir, backup.as_deref()) {
        staging.abort();
        return Err(e);
    }
    let mut bundle = conversion.bundle;
    bundle.legacy_rows.extend(rows);
    bundle.report = report::stored_report(
        &conversion.schema,
        warnings.list(),
        &file_stats,
        backup.as_deref(),
        &staging.name(),
    );
    let mode = match &backup {
        Some(dir) => ImportMode::Replace {
            backup_to: dir.join("mywork.db"),
        },
        None => ImportMode::IntoEmpty,
    };
    // С этого места импорт не отменяется: остановка программы ждёт его.
    options.emit(ImportStage::Writing, 0);
    #[cfg(feature = "testing")]
    {
        let root = folder.root.clone();
        tokio::task::spawn_blocking(move || testing::pause(&root, ImportStage::Writing))
            .await
            .ok();
    }
    let summary = match core.import_bundle(actor, bundle, mode).await {
        Ok(summary) => summary,
        Err(e) => {
            staging.abort();
            tracing::info!("импорт: запись в базу не удалась, промежуточная папка удалена");
            return Err(e.into());
        }
    };
    tracing::info!(
        orders = summary.orders,
        clients = summary.clients,
        history = summary.history,
        "импорт: данные записаны в базу"
    );

    let mut report = ImportReport {
        started_at,
        source: options.source.clone(),
        schema: conversion.schema,
        summary,
        files: file_stats,
        backup,
        warnings: warnings.list().to_vec(),
    };
    let staging_path = staging.path().to_path_buf();
    // Отметка с номером импорта. Не записалась — установка всё равно идёт: если и она оборвётся,
    // запуск сервера доведёт её по предварительной отметке и имени папки в отчёте импорта.
    if let Err(e) = staging.mark_written(report.summary.import_id) {
        tracing::warn!("импорт: отметка промежуточной папки с номером импорта не записана: {e}");
    }
    options.emit(ImportStage::Writing, 100);
    options.emit(ImportStage::Placing, 0);
    let attachments_dir = options.attachments_dir.clone();
    let placing = tokio::task::spawn_blocking(move || {
        let mut after = report::Warnings::default();
        let placed = staging.commit(&attachments_dir, &mut after);
        (placed, after)
    })
    .await;
    // Задача установки не дошла до конца: база записана, файлы остались в промежуточной папке.
    let (placed, mut after) = placing.unwrap_or_else(|e| {
        let error = interrupted("Установка файлов прервана")(e);
        (Err(error), report::Warnings::default())
    });
    if let Err(e) = &placed {
        after.push(
            WarningCode::FileUnreadable,
            "file",
            &staging_path.display().to_string(),
            "",
            "",
            &format!("Файлы не поставлены на место: {e}."),
        );
    }
    report.warnings.extend(after.list().iter().cloned());
    if placed.is_err() {
        tracing::info!("импорт: файлы не поставлены на место");
        return Err(LegacyError::FilesNotPlaced {
            staging: staging_path,
            report: Box::new(report),
        });
    }
    tracing::info!("импорт: файлы поставлены на место");
    options.emit(ImportStage::Placing, 100);
    options.emit(ImportStage::Done, 100);
    Ok(report)
}

/// При запуске сервера: разбирает промежуточные папки оборванных импортов в `attachments_dir`.
/// Папку последнего импорта в базе доустанавливает, остальные удаляет (см. `StagingCleanup`).
/// Если базу прочитать не удалось, папки не трогает.
pub async fn finish_interrupted_imports(
    core: &Core,
    attachments_dir: &Path,
) -> Result<StagingCleanup, LegacyError> {
    let latest = core
        .list_legacy_imports(&Actor::unrestricted("Запуск MyWork 3"))
        .await?
        .into_iter()
        .next()
        .map(|r| files::LatestImport {
            id: r.id,
            staging: r.report["staging"].as_str().map(str::to_owned),
        });
    let dir = attachments_dir.to_path_buf();
    tokio::task::spawn_blocking(move || files::finish_staging(&dir, latest.as_ref()))
        .await
        .map_err(interrupted("Разбор промежуточных папок прерван"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Окончательная отметка не записалась (на месте её временного файла — папка): файлы всё равно
    /// ставятся, по предварительной отметке и имени папки в базе их доустановил бы и запуск.
    #[tokio::test]
    async fn failed_final_mark_still_places_the_files() {
        use std::sync::Arc;
        for (label, db) in mw_db::testing::all_backends().await {
            let core = Core::open(db, Arc::new(mw_core::SystemClock), "UTC")
                .await
                .unwrap();
            let src = test_support::fixture(fixture::FixtureVersion::V32).await;
            let data = tempfile::tempdir().unwrap();
            let att = data.path().join("attachments");
            let watched = att.clone();
            let options = ImportOptions {
                source: src.path().to_path_buf(),
                attachments_dir: att.clone(),
                backups_dir: backups_dir(data.path()),
                replace: false,
                progress: Some(Arc::new(move |stage, percent| {
                    if stage == ImportStage::Writing && percent == 0 {
                        for entry in std::fs::read_dir(&watched).unwrap() {
                            let dir = entry.unwrap().path();
                            if dir
                                .file_name()
                                .unwrap()
                                .to_string_lossy()
                                .starts_with(".import-")
                            {
                                std::fs::create_dir(dir.join(format!("{}.tmp", files::MARK)))
                                    .unwrap();
                            }
                        }
                    }
                })),
                cancel: None,
            };
            let r = import_folder(&core, &Actor::unrestricted("Тест"), &options).await;
            assert!(r.is_ok(), "{label}: {r:?}");
            assert!(att.join("orders/1/photo.jpg").exists(), "{label}");
            let left: Vec<_> = std::fs::read_dir(&att)
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .filter(|n| n.starts_with(".import"))
                .collect();
            assert!(left.is_empty(), "{label}: {left:?}");
        }
    }

    #[test]
    fn lies_inside_resolves_dotdot_in_missing_tail_and_fails_closed() {
        let outer = tempfile::tempdir().unwrap();
        let old = outer.path().join("old");
        std::fs::create_dir_all(&old).unwrap();
        // `..` после несуществующей папки не должно скрывать путь внутри старой папки.
        assert!(lies_inside(&old.join("missing/../x"), &old));
        assert!(lies_inside(&old.join("a/b/../../x/./y"), &old));
        // Выход из старой папки через `..` — снаружи.
        assert!(!lies_inside(&old.join("missing/../../other"), &old));
        assert!(!lies_inside(&outer.path().join("data"), &old));
        // Не удалось раскрыть саму старую папку — отказ.
        assert!(lies_inside(
            &outer.path().join("data"),
            &outer.path().join("nope")
        ));
        // Файл `baza.sqlite` вместо папки.
        std::fs::write(old.join("baza.sqlite"), b"x").unwrap();
        assert!(lies_inside_source(
            &old.join("data"),
            &old.join("baza.sqlite")
        ));
        assert!(!lies_inside_source(
            &outer.path().join("data"),
            &old.join("baza.sqlite")
        ));
    }
}
