//! Проверка папки MyWork 2 перед переносом: только чтение, ничего не записывается.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::LegacyError;
use crate::{convert, files, folder, lies_inside_source, source};

/// Что нашлось в папке MyWork 2 или почему из неё нельзя перенести.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct FolderCheck {
    /// Папку можно переносить.
    pub ok: bool,
    /// Что не так, по-русски (только при `ok = false`).
    #[schema(required = true)]
    pub problem: Option<String>,
    /// Версия базы: «колонок в заказах — 32».
    #[schema(required = true)]
    pub schema_version: Option<String>,
    /// Заказов будет перенесено.
    pub orders: u64,
    /// Клиентов будет перенесено (с восстановленными из заказов).
    pub clients: u64,
    /// Файлов найдено для переноса.
    pub files: u64,
    /// Их общий объём в байтах.
    pub files_bytes: u64,
}

impl FolderCheck {
    fn problem(text: String) -> FolderCheck {
        FolderCheck {
            ok: false,
            problem: Some(text),
            schema_version: None,
            orders: 0,
            clients: 0,
            files: 0,
            files_bytes: 0,
        }
    }
}

/// Проверяет папку MyWork 2 (или файл `baza.sqlite` в ней) для переноса в папку данных `data_dir`:
/// читает базу и файлы и считает, что будет перенесено. Ни в папку MyWork 2, ни в папку данных
/// ничего не пишет (база читается из копии во временной папке). При переносе в пустую базу
/// (`replace = false`) папки вложений `orders`, `stock` и `legacy` должны быть пусты, иначе это проблема;
/// при замене прежние файлы уходят в копию.
pub async fn check_folder(source: &Path, data_dir: &Path, replace: bool) -> FolderCheck {
    match check(source, data_dir, replace).await {
        Ok(found) => found,
        Err(problem) => FolderCheck::problem(problem),
    }
}

fn same_dir(a: &Path, b: &Path) -> bool {
    matches!((a.canonicalize(), b.canonicalize()), (Ok(a), Ok(b)) if a == b)
}

async fn check(source: &Path, data_dir: &Path, replace: bool) -> Result<FolderCheck, String> {
    if !source.is_absolute() {
        return Err("Укажите полный путь к папке MyWork 2".into());
    }
    if !source.exists() {
        return Err(format!("Папка не найдена: {}", source.display()));
    }
    if same_dir(source, data_dir) {
        return Err("Выбрана папка данных MyWork 3, а нужна папка MyWork 2".into());
    }
    let scan_source = source.to_path_buf();
    let folder = tokio::task::spawn_blocking(move || folder::scan_folder(&scan_source))
        .await
        .map_err(|e| format!("Проверка прервана: {e}"))?
        .map_err(|e| match e {
            LegacyError::NoDatabase(_) => {
                "В папке нет baza.sqlite: выберите папку MyWork 2, в которой лежит этот файл".into()
            }
            other => other.to_string(),
        })?;
    if lies_inside_source(data_dir, source) {
        return Err(
            "Папка данных MyWork 3 лежит внутри папки MyWork 2: в папку MyWork 2 ничего \
             не записывается, перенос невозможен"
                .into(),
        );
    }
    if !replace && let Some(dir) = files::occupied_import_dir(&data_dir.join("attachments")) {
        return Err(LegacyError::AttachmentsNotEmpty(dir).to_string());
    }
    let db = source::read_database(&folder.db_file)
        .await
        .map_err(|e| e.to_string())?;
    tokio::task::spawn_blocking(move || {
        // Время подстановок не влияет на числа.
        let conversion = convert::convert(&db, &folder, chrono::NaiveDateTime::default());
        let (files, files_bytes) = files::measure(&folder, &conversion.files);
        FolderCheck {
            ok: true,
            problem: None,
            schema_version: Some(conversion.schema.version_text()),
            orders: conversion.bundle.orders.len() as u64,
            clients: conversion.bundle.clients.len() as u64,
            files,
            files_bytes,
        }
    })
    .await
    .map_err(|e| format!("Проверка прервана: {e}"))
}
