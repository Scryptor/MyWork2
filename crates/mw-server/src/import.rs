//! Команда `import-legacy`: перенос папки MyWork 2 и файлы отчёта.

use crate::config::{Config, ServerError};
use mw_core::{Actor, Core, SystemClock};
use mw_legacy::{ImportOptions, ImportReport, LegacyError, import_folder};
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

/// Сообщение об ошибке импорта; для непустой базы добавляет подсказку.
pub fn import_error_message(error: &LegacyError) -> String {
    match error {
        LegacyError::TargetNotEmpty(_) => format!(
            "{error}. Импорт делается только в пустую базу. Чтобы заменить данные SQLite-базы, \
             запустите команду ещё раз с --replace (сначала будет сделана копия). \
             Для PostgreSQL создайте новую пустую базу."
        ),
        _ => error.to_string(),
    }
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
        backups_dir: config.data_dir.join("backups"),
        replace,
    };
    match import_folder(&core, &actor, &options).await {
        Ok(report) => {
            let (text_report, json_report) = write_reports(config, &core, &report)?;
            let open_mode = core.is_open_mode().await?;
            Ok(ImportOutcome {
                report,
                text_report,
                json_report,
                open_mode,
            })
        }
        Err(LegacyError::FilesNotPlaced { staging, report }) => {
            // База уже импортирована: отчёт всё равно нужен, а файлы остались в промежуточной папке.
            let (text, json) = write_reports(config, &core, &report)?;
            eprintln!("Отчёт: {}", text.display());
            eprintln!("Отчёт (JSON): {}", json.display());
            Err(ServerError::Import(LegacyError::FilesNotPlaced {
                staging,
                report,
            }))
        }
        Err(error) => Err(ServerError::Import(error)),
    }
}

fn write_reports(
    config: &Config,
    core: &Core,
    report: &ImportReport,
) -> Result<(PathBuf, PathBuf), ServerError> {
    let dir = config.data_dir.join("import-reports");
    std::fs::create_dir_all(&dir).map_err(|source| ServerError::Io {
        context: format!("не удалось создать папку {}", dir.display()),
        source,
    })?;
    let stamp = core.now().format("%Y%m%d-%H%M%S").to_string();
    let json = serde_json::to_string_pretty(&report.to_json()).unwrap_or_default();
    // Имя, свободное для обоих файлов; существующие файлы не перезаписываются.
    let mut n = 1;
    loop {
        let base = if n == 1 {
            format!("mywork2-{stamp}")
        } else {
            format!("mywork2-{stamp}-{n}")
        };
        let text_path = dir.join(format!("{base}.txt"));
        let json_path = dir.join(format!("{base}.json"));
        if text_path.exists() || json_path.exists() {
            n += 1;
            continue;
        }
        write_new(&text_path, &report.full_text())?;
        write_new(&json_path, &json)?;
        return Ok((text_path, json_path));
    }
}

fn write_new(path: &Path, content: &str) -> Result<(), ServerError> {
    use std::io::Write;
    let io = |source| ServerError::Io {
        context: format!("не удалось записать отчёт {}", path.display()),
        source,
    };
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(io)?;
    file.write_all(content.as_bytes()).map_err(io)
}
