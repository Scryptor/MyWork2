//! Копии файлов старой папки: сначала в промежуточную папку, на место — только после успешной записи в базу.

use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use mw_core::ImportedLegacyRow;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::LegacyError;
use crate::convert::PlannedFile;
use crate::folder::LegacyFolder;
use crate::report::{FileStats, WarningCode, Warnings};

/// Папки внутри `attachments`, которые занимает импорт.
pub(crate) const IMPORT_DIRS: [&str; 3] = ["orders", "stock", "legacy"];

/// Промежуточная папка `attachments/.import-<время>` со скопированными файлами.
#[derive(Debug)]
pub(crate) struct Staging {
    dir: PathBuf,
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

/// Копирует файлы в `attachments_dir/.import-<stamp>`; возвращает следы для базы и счётчики.
///
/// Источник каждый раз заново ищется через `LegacyFolder::find_file`: путь не выходит за пределы старой папки.
pub(crate) fn stage(
    folder: &LegacyFolder,
    files: &[PlannedFile],
    attachments_dir: &Path,
    stamp: &str,
    warnings: &mut Warnings,
) -> Result<(Staging, Vec<ImportedLegacyRow>, FileStats), LegacyError> {
    std::fs::create_dir_all(attachments_dir).map_err(io_error(format!(
        "Не удалось создать папку {}",
        attachments_dir.display()
    )))?;
    let mut dir = attachments_dir.join(format!(".import-{stamp}"));
    let mut n = 1;
    while dir.exists() {
        n += 1;
        dir = attachments_dir.join(format!(".import-{stamp}-{n}"));
    }
    std::fs::create_dir(&dir).map_err(io_error(format!(
        "Не удалось создать папку {}",
        dir.display()
    )))?;
    let staging = Staging { dir };
    match copy_all(folder, files, &staging, warnings) {
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
) -> Result<(Vec<ImportedLegacyRow>, FileStats), LegacyError> {
    let mut rows = Vec::new();
    let mut stats = FileStats::default();
    let mut used = BTreeSet::new();
    for file in files {
        let shown = file.source.display().to_string();
        let found = file
            .source
            .to_str()
            .and_then(|relative| folder.find_file(relative));
        let (Some(from), true) = (found, is_plain_relative(&file.dest)) else {
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

/// Переносит папку; между дисками — копированием.
fn move_dir(from: &Path, to: &Path) -> Result<(), LegacyError> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    copy_dir(from, to)?;
    std::fs::remove_dir_all(from).map_err(io_error(format!(
        "Не удалось убрать папку {}",
        from.display()
    )))
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

impl Staging {
    /// Удаляет промежуточную папку со всем содержимым.
    pub(crate) fn abort(self) {
        std::fs::remove_dir_all(&self.dir).ok();
    }

    /// Ставит файлы на место. С `previous` (папка копии) прежние `orders`, `stock` и `legacy` уходят туда.
    pub(crate) fn commit(
        self,
        attachments_dir: &Path,
        previous: Option<&Path>,
    ) -> Result<(), LegacyError> {
        if let Some(previous) = previous {
            for name in IMPORT_DIRS {
                let old = attachments_dir.join(name);
                if old.exists() {
                    std::fs::create_dir_all(previous).map_err(io_error(format!(
                        "Не удалось создать папку {}",
                        previous.display()
                    )))?;
                    move_dir(&old, &previous.join(name))?;
                }
            }
        }
        for name in IMPORT_DIRS {
            let staged = self.dir.join(name);
            if staged.exists() {
                let target = attachments_dir.join(name);
                if target.exists() {
                    return Err(LegacyError::AttachmentsNotEmpty(target));
                }
                move_dir(&staged, &target)?;
            }
        }
        std::fs::remove_dir_all(&self.dir).map_err(io_error(format!(
            "Не удалось убрать папку {}",
            self.dir.display()
        )))
    }
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
        let (staging, rows, stats) =
            stage(&folder, &files, attachments.path(), "x", &mut w).unwrap();
        assert_eq!((stats.copied, stats.missing), (2, 2));
        assert_eq!(rows[1].raw["stored_as"], "attachments/orders/1/A (2).jpg");
        assert_eq!(w.list().len(), 2);
        staging.abort();
    }
}
