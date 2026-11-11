//! Выгрузка TypeScript-типов API.

use std::path::Path;

use ts_rs::TS;

use crate::{ErrorBody, ServerInfo};

/// Выгружает в `dir` TS-типы всех корневых типов API вместе с зависимостями
/// (по файлу на тип). `i64` выгружается как `number`: суммы и id меньше 2^53.
///
/// `dir` должна быть новой пустой папкой: ts-rs дописывает в существующие файлы.
pub fn export_typescript(dir: &Path) -> Result<(), ts_rs::ExportError> {
    let cfg = ts_rs::Config::new()
        .with_large_int("number")
        .with_out_dir(dir);
    // Единый список корневых типов: каждая следующая задача добавляет сюда
    // свои типы тел, ответов и параметров запросов.
    ErrorBody::export_all(&cfg)?;
    ServerInfo::export_all(&cfg)?;
    Ok(())
}
