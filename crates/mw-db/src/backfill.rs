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
///
/// Вызывается из выпущенной миграции, поэтому не зависит от набора колонок сущностей:
/// читает и пишет только перечисленные здесь колонки.
pub async fn legacy_client_files(conn: &impl ConnectionTrait) -> Result<u64, DbErr> {
    use std::collections::{HashMap, HashSet};

    use sea_orm::sea_query::{Alias, Query};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

    use crate::entity::{attachment, legacy_import, legacy_row};

    type Trace = (i64, Option<i64>, String);
    let rows: Vec<Trace> = legacy_row::Entity::find()
        .select_only()
        .column(legacy_row::Column::ImportId)
        .column(legacy_row::Column::TargetId)
        .column(legacy_row::Column::Raw)
        .filter(legacy_row::Column::Source.eq("ClientFiles"))
        .filter(legacy_row::Column::TargetTable.eq("orders"))
        .order_by_asc(legacy_row::Column::Id)
        .into_tuple()
        .all(conn)
        .await?;
    if rows.is_empty() {
        return Ok(0);
    }
    let mut known: HashSet<String> = attachment::Entity::find()
        .select_only()
        .column(attachment::Column::Path)
        .into_tuple::<String>()
        .all(conn)
        .await?
        .into_iter()
        .collect();
    let imported_at: HashMap<i64, sea_orm::entity::prelude::DateTime> =
        legacy_import::Entity::find()
            .select_only()
            .column(legacy_import::Column::Id)
            .column(legacy_import::Column::ImportedAt)
            .into_tuple()
            .all(conn)
            .await?
            .into_iter()
            .collect();

    let mut pending: Vec<[sea_orm::sea_query::SimpleExpr; 9]> = Vec::new();
    for (import_id, target_id, raw) in rows {
        let (Some(order_id), Some(at)) = (target_id, imported_at.get(&import_id)) else {
            continue;
        };
        let Ok(raw) = serde_json::from_str::<serde_json::Value>(&raw) else {
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
        pending.push([
            order_id.into(),
            kind.into(),
            name.into(),
            "".into(),
            content_type.into(),
            raw.get("size").and_then(|v| v.as_i64()).unwrap_or(0).into(),
            path.to_owned().into(),
            true.into(),
            (*at).into(),
        ]);
    }
    let added = pending.len() as u64;
    for chunk in pending.chunks(500) {
        let mut insert = Query::insert();
        insert.into_table(Alias::new("attachments")).columns([
            Alias::new("order_id"),
            Alias::new("kind"),
            Alias::new("name"),
            Alias::new("caption"),
            Alias::new("content_type"),
            Alias::new("size"),
            Alias::new("path"),
            Alias::new("legacy"),
            Alias::new("created_at"),
        ]);
        for values in chunk {
            insert.values_panic(values.clone());
        }
        conn.execute(&insert).await?;
    }
    Ok(added)
}
