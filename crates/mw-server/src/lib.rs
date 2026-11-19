//! Сервер MyWork: настройка, запуск, остановка и создание первого администратора.
//! Библиотеку использует и тонкий бинарник `mw-server`, и (позже) десктоп-приложение.

pub mod admin;
pub mod cli;
pub mod config;
pub mod import;
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
pub use paths::{DataDirOs, os_data_dir, os_data_dir_for, resolve_data_dir};
pub use server::{Server, shutdown_signal};
