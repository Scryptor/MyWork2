//! Проверка папки MyWork 2 перед переносом: только чтение, ничего не записывается.

use std::path::Path;

use mw_core::{Core, CoreError, DatabaseKind, ImportTarget};
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
    /// В базе уже есть данные, и перенести можно только с заменой (SQLite): интерфейс предлагает её.
    pub replace_required: bool,
    /// Куда при замене уйдёт копия нынешних данных (только при проверке с заменой).
    #[schema(required = true)]
    pub backups_dir: Option<String>,
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
            replace_required: false,
            backups_dir: None,
        }
    }
}

/// Помеха со стороны базы MyWork 3, до чтения папки: без замены база должна быть пустой (в ней мог уже
/// появиться администратор — `mw-server create-admin`), замена есть только у SQLite. `None` — помехи нет.
pub async fn check_target(core: &Core, replace: bool) -> Result<Option<FolderCheck>, CoreError> {
    let sqlite = core.database_kind() == DatabaseKind::Sqlite;
    if replace {
        return Ok((!sqlite).then(|| {
            FolderCheck::problem(format!(
                "{}: перенести можно только в новую пустую базу PostgreSQL",
                LegacyError::ReplaceUnsupported
            ))
        }));
    }
    let target = core.import_target_state().await?;
    if target.is_empty() {
        return Ok(None);
    }
    let remedy = if sqlite {
        "Перенести можно только с заменой: нынешние данные сохранятся копией"
    } else {
        "Перенести можно только в пустую базу: создайте новую пустую базу PostgreSQL"
    };
    Ok(Some(FolderCheck {
        replace_required: sqlite,
        ..FolderCheck::problem(format!(
            "В базе MyWork 3 уже есть данные ({}). {remedy}",
            existing(&target)
        ))
    }))
}

/// Что уже лежит в базе — только ненулевые числа (те же, что называет отказ импорта).
fn existing(target: &ImportTarget) -> String {
    [
        ("клиенты", target.clients),
        ("заказы", target.orders),
        ("пользователи", target.users),
        ("записи журнала", target.audit_entries),
    ]
    .into_iter()
    .filter(|(_, count)| *count > 0)
    .map(|(name, count)| format!("{name}: {count}"))
    .collect::<Vec<_>>()
    .join(", ")
}

/// Проверяет папку MyWork 2 (или файл `baza.sqlite` в ней) для переноса в папку данных `data_dir`:
/// читает базу и файлы и считает, что будет перенесено. Ни в папку MyWork 2, ни в папку данных
/// ничего не пишет (база читается из копии во временной папке). При переносе в пустую базу
/// (`replace = false`) папки вложений `orders`, `stock` и `legacy` должны быть пусты, иначе это проблема;
/// при замене прежние файлы уходят в копию (`backups_dir`). Состояние базы проверяет `check_target`.
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
    #[cfg(feature = "testing")]
    {
        let root = source.to_path_buf();
        let _ = tokio::task::spawn_blocking(move || crate::testing::pause_check(&root)).await;
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
    let backups = data_dir.join("backups");
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
            replace_required: false,
            backups_dir: replace.then(|| backups.display().to_string()),
        }
    })
    .await
    .map_err(|e| format!("Проверка прервана: {e}"))
}
