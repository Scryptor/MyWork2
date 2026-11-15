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

/// Импортирует папку MyWork 2: читает, преобразует, копирует файлы через промежуточную папку,
/// пишет пакет одной транзакцией и только после успеха ставит файлы на место.
pub async fn import_folder(
    core: &Core,
    actor: &Actor,
    options: &ImportOptions,
) -> Result<ImportReport, LegacyError> {
    let folder = folder::scan_folder(&options.source)?;
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

    let previous = backup.as_ref().map(|dir| dir.join("attachments"));
    staging.commit(&options.attachments_dir, previous.as_deref())?;
    tracing::info!("импорт: файлы поставлены на место");

    Ok(ImportReport {
        started_at,
        source: options.source.clone(),
        schema: conversion.schema,
        summary,
        files: file_stats,
        backup,
        warnings: warnings.list().to_vec(),
    })
}
