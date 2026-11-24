use std::path::PathBuf;

use mw_core::{CoreError, ImportTarget};

use crate::report::ImportReport;

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
        "В базе MyWork 3 уже есть данные (клиенты: {}, заказы: {}, пользователи: {}, записи журнала: {}, \
         позиции склада: {})",
        .0.clients, .0.orders, .0.users, .0.audit_entries, .0.stock_items
    )]
    TargetNotEmpty(ImportTarget),
    #[error("Папка вложений не пуста: {0}")]
    AttachmentsNotEmpty(PathBuf),
    #[error("Замена данных для этой базы не поддерживается")]
    ReplaceUnsupported,
    #[error("Папка {0} лежит внутри старой папки MyWork 2: в неё ничего не записывается")]
    InsideSource(PathBuf),
    #[error("Импорт прерван: программа закрывается")]
    Cancelled,
    #[error(
        "База импортирована, но файлы не поставлены на место: они лежат в папке {}",
        .staging.display()
    )]
    FilesNotPlaced {
        staging: PathBuf,
        report: Box<ImportReport>,
    },
}

/// Откуда запущен импорт: от этого зависит подсказка в сообщении об ошибке.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartedFrom {
    /// Команда `mw-server import-legacy`.
    Command,
    /// Интерфейс программы.
    Interface,
}

/// Сообщение об ошибке импорта; для непустой базы добавляет подсказку, как заменить данные.
pub fn import_error_message(error: &LegacyError, from: StartedFrom) -> String {
    match error {
        LegacyError::TargetNotEmpty(_) => {
            let replace = match from {
                StartedFrom::Command => "запустите команду ещё раз с --replace",
                StartedFrom::Interface => "перенесите данные с заменой",
            };
            format!(
                "{error}. Импорт делается только в пустую базу. Чтобы заменить данные SQLite-базы, \
                 {replace} (сначала будет сделана копия). Для PostgreSQL создайте новую пустую базу."
            )
        }
        // Интерфейс не предлагает переносить файлы руками: запуск сервера доустановит их сам.
        LegacyError::FilesNotPlaced { .. } if from == StartedFrom::Interface => {
            "Файлы не удалось поставить на место — перезапустите программу, она закончит перенос сама"
                .to_owned()
        }
        _ => error.to_string(),
    }
}
