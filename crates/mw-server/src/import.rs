//! Команда `import-legacy`: перенос папки MyWork 2 и файлы отчёта.

use crate::config::{Config, ServerError};
use mw_core::{Actor, Core, SystemClock};
use mw_legacy::{ImportOptions, ImportReport, LegacyError, StartedFrom, import_folder};
use sea_orm::DatabaseConnection;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::server::system_timezone;

/// Итог импорта: отчёт и пути записанных файлов отчёта.
#[derive(Debug)]
pub struct ImportOutcome {
    pub report: ImportReport,
    pub text_report: PathBuf,
    pub json_report: PathBuf,
    /// После импорта в базе нет пользователей (вход без пароля).
    pub open_mode: bool,
}

/// Сообщение об ошибке импорта командой; для непустой базы добавляет подсказку про `--replace`.
pub fn import_error_message(error: &LegacyError) -> String {
    mw_legacy::import_error_message(error, StartedFrom::Command)
}

/// Проверяет до подключения к базе, что папка данных и файл SQLite лежат вне старой папки MyWork 2:
/// подключение создаёт файлы, а в старую папку ничего не пишется.
pub fn refuse_inside_source(config: &Config, source: &Path) -> Result<(), ServerError> {
    let mut targets = vec![config.data_dir.clone()];
    let url = config.database_url();
    if let Ok(parsed) = url::Url::parse(&url)
        && parsed.scheme() == "sqlite"
        && let Ok(file) = parsed.to_file_path()
    {
        targets.push(file);
    }
    for target in targets {
        if mw_legacy::lies_inside_source(&target, source) {
            return Err(ServerError::Import(LegacyError::InsideSource(target)));
        }
    }
    Ok(())
}

/// Открывает `Core` на готовой базе, переносит папку и пишет отчёты в `data_dir/import-reports`.
pub async fn run_import_with_db(
    config: &Config,
    db: DatabaseConnection,
    source: &Path,
    replace: bool,
) -> Result<ImportOutcome, ServerError> {
    let core = Core::open(db, Arc::new(SystemClock), &system_timezone(config)).await?;
    let actor = Actor::unrestricted("Импорт MyWork 2");
    let options = ImportOptions {
        source: source.to_path_buf(),
        attachments_dir: config.data_dir.join("attachments"),
        backups_dir: mw_legacy::backups_dir(&config.data_dir),
        replace,
        progress: None,
        cancel: None,
    };
    match import_folder(&core, &actor, &options).await {
        Ok(report) => match write_reports(config, &core, &report) {
            Ok((text_report, json_report)) => {
                let open_mode = core.is_open_mode().await?;
                Ok(ImportOutcome {
                    report,
                    text_report,
                    json_report,
                    open_mode,
                })
            }
            Err(error) => Err(ServerError::ImportReportNotWritten {
                report: Box::new(report),
                reason: error.to_string(),
                staging: None,
            }),
        },
        Err(LegacyError::FilesNotPlaced { staging, report }) => {
            // База уже импортирована: отчёт всё равно нужен, а файлы остались в промежуточной папке.
            match write_reports(config, &core, &report) {
                Ok((text_report, json_report)) => Err(ServerError::ImportFilesNotPlaced {
                    staging,
                    text_report,
                    json_report,
                    report,
                }),
                Err(error) => Err(ServerError::ImportReportNotWritten {
                    report,
                    reason: error.to_string(),
                    staging: Some(staging),
                }),
            }
        }
        Err(error) => Err(ServerError::Import(error)),
    }
}

/// Файлы отчёта в `data_dir/import-reports` (общий код с импортом из интерфейса).
fn write_reports(
    config: &Config,
    core: &Core,
    report: &ImportReport,
) -> Result<(PathBuf, PathBuf), LegacyError> {
    mw_legacy::write_report_files(&config.data_dir, core.now(), report)
}
