//! Чтение копии базы и папки MyWork 2 и перенос их в MyWork 3.

mod convert;
mod error;
mod files;
mod folder;
mod parse;
mod report;
mod source;
mod text;

#[cfg(feature = "fixtures")]
pub mod fixture;

#[cfg(test)]
mod test_support;

use std::path::{Path, PathBuf};

use mw_core::{Actor, Core, DatabaseKind, ImportMode};

pub use error::LegacyError;
pub use report::{FileStats, ImportReport, Severity, Warning, WarningCode};
pub use source::SchemaSummary;

/// Что и куда импортировать.
#[derive(Debug, Clone)]
pub struct ImportOptions {
    /// Папка MyWork 2 (только чтение).
    pub source: PathBuf,
    /// Папка вложений MyWork 3.
    pub attachments_dir: PathBuf,
    /// Папка копий; при замене в ней появляется `<время>/`.
    pub backups_dir: PathBuf,
    /// Заменить уже имеющиеся данные (только SQLite), предварительно сняв копию.
    pub replace: bool,
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
pub async fn import_folder(
    core: &Core,
    actor: &Actor,
    options: &ImportOptions,
) -> Result<ImportReport, LegacyError> {
    core.check_import_rights(actor)?;
    let folder = folder::scan_folder(&options.source)?;
    for dir in [&options.attachments_dir, &options.backups_dir] {
        if lies_inside(dir, &folder.root) {
            return Err(LegacyError::InsideSource(dir.clone()));
        }
    }
    let db = source::read_database(&folder.db_file).await?;
    tracing::info!(tables = db.tables.len(), "импорт: старая база прочитана");

    if options.replace && core.database_kind() != DatabaseKind::Sqlite {
        return Err(LegacyError::ReplaceUnsupported);
    }
    if !options.replace {
        let target = core.import_target_state().await?;
        if !target.is_empty() {
            return Err(LegacyError::TargetNotEmpty(target));
        }
        for name in files::IMPORT_DIRS {
            let dir = options.attachments_dir.join(name);
            if files::dir_has_entries(&dir) {
                return Err(LegacyError::AttachmentsNotEmpty(dir));
            }
        }
    }

    let started_at = core.now();
    let stamp = started_at.format("%Y%m%d-%H%M%S").to_string();
    let mut conversion = convert::convert(&db, &folder, started_at);
    tracing::info!(
        orders = conversion.bundle.orders.len(),
        clients = conversion.bundle.clients.len(),
        history = conversion.bundle.history.len(),
        files = conversion.files.len(),
        "импорт: данные преобразованы"
    );

    let attachments_dir = options.attachments_dir.clone();
    let stage_stamp = stamp.clone();
    let (staging, rows, file_stats, warnings) = tokio::task::spawn_blocking({
        let folder = folder.clone();
        let planned = std::mem::take(&mut conversion.files);
        let mut warnings = std::mem::take(&mut conversion.warnings);
        move || {
            let staged = files::stage(
                &folder,
                &planned,
                &attachments_dir,
                &stage_stamp,
                &mut warnings,
            );
            staged.map(|(s, r, f)| (s, r, f, warnings))
        }
    })
    .await
    .map_err(|e| LegacyError::Io {
        context: "Копирование файлов прервано".to_owned(),
        source: std::io::Error::other(e),
    })??;
    tracing::info!(
        copied = file_stats.copied,
        missing = file_stats.missing,
        bytes = file_stats.bytes,
        "импорт: файлы скопированы в промежуточную папку"
    );

    let mut bundle = conversion.bundle;
    bundle.legacy_rows.extend(rows);
    bundle.report = report::stored_report(&conversion.schema, warnings.list(), &file_stats);

    let backup = options
        .replace
        .then(|| backup_folder(&options.backups_dir, &stamp));
    let mode = match &backup {
        Some(dir) => ImportMode::Replace {
            backup_to: dir.join("mywork.db"),
        },
        None => ImportMode::IntoEmpty,
    };
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
    let previous = report.backup.as_ref().map(|dir| dir.join("attachments"));
    let staging_path = staging.path().to_path_buf();
    let mut after = report::Warnings::default();
    let placed = staging.commit(&options.attachments_dir, previous.as_deref(), &mut after);
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
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

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
