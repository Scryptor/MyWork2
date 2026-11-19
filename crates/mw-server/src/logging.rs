//! Журнал сервера: всегда в стандартный вывод, а при заданной папке — ещё и в файл
//! `mywork.<дата>.log` со сменой раз в сутки (хранятся 14 последних файлов).

use std::path::Path;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{Builder, Rotation};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::writer::MakeWriterExt;

use crate::config::ServerError;

/// Сколько файлов журнала хранится.
pub const KEPT_LOG_FILES: usize = 14;

/// Держит запись в файл: при удалении дописывает то, что осталось в буфере.
#[must_use = "без охранника журнал в файл не дописывается при выходе"]
pub struct LogGuard {
    _file: Option<WorkerGuard>,
}

/// Подключает журнал. Фильтр — из `MW_LOG`, по умолчанию `info,sqlx=warn`. Повторный вызов в том же
/// процессе не меняет уже подключённый журнал.
pub fn init_logging(log_dir: Option<&Path>) -> Result<LogGuard, ServerError> {
    let filter =
        EnvFilter::try_from_env("MW_LOG").unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn"));
    let Some(dir) = log_dir else {
        let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
        return Ok(LogGuard { _file: None });
    };
    let io_error = |source| ServerError::Io {
        context: format!("не удалось подготовить папку журнала {}", dir.display()),
        source,
    };
    std::fs::create_dir_all(dir).map_err(io_error)?;
    let appender = Builder::new()
        .rotation(Rotation::DAILY)
        .filename_prefix("mywork")
        .filename_suffix("log")
        .max_log_files(KEPT_LOG_FILES)
        .build(dir)
        .map_err(|e| {
            ServerError::Config(format!(
                "не удалось открыть журнал в {}: {e}",
                dir.display()
            ))
        })?;
    let (file, guard) = tracing_appender::non_blocking(appender);
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stdout.and(file))
        .with_ansi(false)
        .try_init();
    Ok(LogGuard { _file: Some(guard) })
}
