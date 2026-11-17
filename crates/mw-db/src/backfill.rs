//! Пересчёт производных колонок по журналу.

use sea_orm::{ConnectionTrait, DbErr, Statement};

/// `orders.status_changed_at` = время последней смены статуса из журнала (в том числе
/// записи импорта `legacy.status`); без записей — пусто. Возвращает число обновлённых строк.
pub async fn status_changed_at(conn: &impl ConnectionTrait) -> Result<u64, DbErr> {
    let sql = "UPDATE orders SET status_changed_at = (SELECT MAX(a.at) FROM audit_log a \
               WHERE a.order_id = orders.id AND a.action IN ('order.status_changed', 'legacy.status'))";
    let res = conn
        .execute_raw(Statement::from_string(conn.get_database_backend(), sql))
        .await?;
    Ok(res.rows_affected())
}

/// Расширения, которые считаются фотографиями.
const PHOTO_EXTENSIONS: [&str; 6] = ["jpg", "jpeg", "png", "webp", "gif", "bmp"];

/// Вид и тип содержимого по имени файла: `("photo" | "file", mime)`.
pub fn classify_file(name: &str) -> (&'static str, String) {
    let extension = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_lowercase())
        .unwrap_or_default();
    let kind = if PHOTO_EXTENSIONS.contains(&extension.as_str()) {
        "photo"
    } else {
        "file"
    };
    let content_type = mime_guess::from_path(name)
        .first_raw()
        .unwrap_or("application/octet-stream")
        .to_owned();
    (kind, content_type)
}

/// Вложения по следам файлов старой программы (`legacy_rows`: `ClientFiles` → `orders`).
/// Время вложения — время импорта. Повторный запуск не дублирует (по `path`); следы без
/// `stored_as` пропускаются. Возвращает число добавленных вложений.
pub async fn legacy_client_files(conn: &impl ConnectionTrait) -> Result<u64, DbErr> {
    use std::collections::{HashMap, HashSet};

    use sea_orm::{ActiveValue::NotSet, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};

    use crate::entity::{attachment, legacy_import, legacy_row};

    let rows = legacy_row::Entity::find()
        .filter(legacy_row::Column::Source.eq("ClientFiles"))
        .filter(legacy_row::Column::TargetTable.eq("orders"))
        .order_by_asc(legacy_row::Column::Id)
        .all(conn)
        .await?;
    if rows.is_empty() {
        return Ok(0);
    }
    let mut known: HashSet<String> = attachment::Entity::find()
        .all(conn)
        .await?
        .into_iter()
        .map(|a| a.path)
        .collect();
    let imported_at: HashMap<i64, _> = legacy_import::Entity::find()
        .all(conn)
        .await?
        .into_iter()
        .map(|i| (i.id, i.imported_at))
        .collect();
    let mut fresh = Vec::new();
    for row in rows {
        let (Some(order_id), Some(at)) = (row.target_id, imported_at.get(&row.import_id)) else {
            continue;
        };
        let Ok(raw) = serde_json::from_str::<serde_json::Value>(&row.raw) else {
            continue;
        };
        let Some(path) = raw.get("stored_as").and_then(|v| v.as_str()) else {
            continue;
        };
        if !known.insert(path.to_owned()) {
            continue;
        }
        let name = path.rsplit('/').next().unwrap_or(path).to_owned();
        let (kind, content_type) = classify_file(&name);
        fresh.push(attachment::ActiveModel {
            id: NotSet,
            order_id: Set(Some(order_id)),
            draft_key: Set(None),
            kind: Set(kind.to_owned()),
            name: Set(name),
            caption: Set(String::new()),
            content_type: Set(content_type),
            size: Set(raw.get("size").and_then(|v| v.as_i64()).unwrap_or(0)),
            width: Set(None),
            height: Set(None),
            path: Set(path.to_owned()),
            thumb_path: Set(None),
            legacy: Set(true),
            created_at: Set(*at),
        });
    }
    let added = fresh.len() as u64;
    for chunk in fresh.chunks(500) {
        attachment::Entity::insert_many(chunk.to_vec())
            .exec_without_returning(conn)
            .await?;
    }
    Ok(added)
}
