use std::path::PathBuf;

use mw_core::{CoreError, ImportTarget};

/// Ошибки чтения старой папки и импорта.
#[derive(Debug, thiserror::Error)]
pub enum LegacyError {
    #[error("Папка или файл не найдены: {0}")]
    SourceNotFound(PathBuf),
    #[error("В папке нет файла базы baza.sqlite: {0}")]
    NoDatabase(PathBuf),
    #[error("Это не база MyWork 2: нет таблицы Catalog")]
    NotMyWork2,
    #[error("{context}: {source}")]
    Io {
        context: String,
        source: std::io::Error,
    },
    #[error("Ошибка чтения базы: {0}")]
    Sqlite(String),
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error(
        "В базе MyWork 3 уже есть данные (клиенты: {}, заказы: {}, пользователи: {}, записи журнала: {})",
        .0.clients, .0.orders, .0.users, .0.audit_entries
    )]
    TargetNotEmpty(ImportTarget),
    #[error("Папка вложений не пуста: {0}")]
    AttachmentsNotEmpty(PathBuf),
    #[error("Замена данных для этой базы не поддерживается")]
    ReplaceUnsupported,
}
