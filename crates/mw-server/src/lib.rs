//! Сервер MyWork: настройка, запуск, остановка и создание первого администратора.
//! Библиотеку использует и тонкий бинарник `mw-server`, и программа для компьютера (`apps/desktop`).

pub mod admin;
pub mod cli;
pub mod config;
pub mod import;
pub mod lock;
pub mod logging;
pub mod paths;
pub mod server;

pub use admin::{create_admin, read_password};
pub use cli::{Cli, Command, run};
pub use config::{
    Config, DATA_DIR_NAME, DB_FILE_NAME, DEFAULT_LISTEN, ServerError, redact_database_url,
};
pub use import::{ImportOutcome, run_import_with_db};
pub use logging::{LogGuard, init_logging};
/// Режим процесса-рендерера: `main` обеих программ проверяет этот первый аргумент до всего остального.
pub use mw_docs::{WORKER_ARG as RENDER_WORKER_ARG, worker_main as run_render_worker};
pub use paths::{
    DataDirOs, PORTABLE_FILE, data_dir_name, os_data_dir, os_data_dir_for, portable_data_dir,
    resolve_data_dir,
};
pub use server::{Server, shutdown_signal};
