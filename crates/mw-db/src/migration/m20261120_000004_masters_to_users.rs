//! Мастера — пользователи: имена мастеров из заказов, справочника «Мастера» (и архивные) и
//! «мастера по умолчанию» становятся пользователями-мастерами по правилу `crate::masters`;
//! заказы получают `master_user_id` (текст `master` не меняется), значения справочника «Мастера»
//! уходят в архив, «мастер по умолчанию» — номером (`default_master_user_id`; прежняя строка
//! `default_master` остаётся). Одна сводная запись журнала, если что-то изменилось; повторный
//! запуск ничего не меняет.

use chrono::{SubsecRound, TimeZone, Utc};
use chrono_tz::Tz;
use sea_orm_migration::prelude::*;
use serde_json::json;

use crate::masters::masters_to_users;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261120_000004_masters_to_users"
    }
}

fn a(name: &str) -> Alias {
    Alias::new(name)
}

/// Кто записал сводку в журнал.
const AUTHOR: &str = "Обновление программы";

/// Значение строки настроек (JSON), если она есть.
async fn setting(db: &SchemaManagerConnection<'_>, key: &str) -> Result<Option<String>, DbErr> {
    let mut select = Query::select();
    select
        .column(a("value"))
        .from(a("settings"))
        .and_where(Expr::col(a("key")).eq(key));
    match db.query_one(&select).await? {
        Some(row) => Ok(Some(row.try_get_by_index(0)?)),
        None => Ok(None),
    }
}

/// Местное время предприятия по настройке `timezone` (нет или неверная — UTC), до секунд: так
/// же, как записывает `Core::now()`.
async fn local_now(db: &SchemaManagerConnection<'_>) -> Result<chrono::NaiveDateTime, DbErr> {
    let tz = setting(db, "timezone")
        .await?
        .and_then(|v| serde_json::from_str::<String>(&v).ok())
        .and_then(|name| name.parse::<Tz>().ok())
        .unwrap_or(Tz::UTC);
    Ok(tz
        .from_utc_datetime(&Utc::now().naive_utc())
        .naive_local()
        .trunc_subsecs(0))
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        let now = local_now(db).await?;

        // Написание нового пользователя: сначала справочник (действующие, затем архивные), затем
        // мастер по умолчанию, затем заказы.
        let mut select = Query::select();
        select
            .column(a("value"))
            .from(a("dictionary_items"))
            .and_where(Expr::col(a("kind")).eq("master"))
            .order_by(a("archived"), Order::Asc)
            .order_by(a("sort_order"), Order::Asc)
            .order_by(a("id"), Order::Asc);
        let mut names: Vec<String> = Vec::new();
        for row in db.query_all(&select).await? {
            names.push(row.try_get_by_index(0)?);
        }
        let default_master = setting(db, "default_master")
            .await?
            .and_then(|v| serde_json::from_str::<String>(&v).ok())
            .unwrap_or_default();
        names.push(default_master.clone());

        let masters = masters_to_users(db, &names, now).await?;

        let mut archive = Query::update();
        archive
            .table(a("dictionary_items"))
            .value(a("archived"), true)
            .and_where(Expr::col(a("kind")).eq("master"))
            .and_where(Expr::col(a("archived")).eq(false));
        let archived = db.execute(&archive).await?.rows_affected();

        let mut default_id = None;
        if let Some(id) = masters.user_for(&default_master)
            && setting(db, "default_master_user_id").await?.is_none()
        {
            let mut insert = Query::insert();
            insert
                .into_table(a("settings"))
                .columns([a("key"), a("value")])
                .values([
                    "default_master_user_id".into(),
                    json!(id).to_string().into(),
                ])
                .map_err(|e| DbErr::Migration(format!("мастер по умолчанию: {e}")))?;
            db.execute(&insert).await?;
            default_id = Some(id);
        }

        let changed = !masters.created.is_empty()
            || !masters.marked.is_empty()
            || masters.orders_linked > 0
            || archived > 0
            || default_id.is_some();
        if !changed {
            return Ok(());
        }
        let list = |users: &[(i64, String)]| -> Vec<serde_json::Value> {
            users
                .iter()
                .map(|(id, name)| json!({ "id": id, "name": name }))
                .collect()
        };
        let details = json!({
            "created": list(&masters.created),
            "marked": list(&masters.marked),
            "orders_linked": masters.orders_linked,
            "dictionary_archived": archived,
            "default_master_user_id": default_id,
            "role_created": masters.role_created,
        });
        let summary = format!(
            "Мастера перенесены в пользователей: создано {}, отмечено {}",
            masters.created.len(),
            masters.marked.len()
        );
        let mut insert = Query::insert();
        insert
            .into_table(a("audit_log"))
            .columns([
                a("at"),
                a("user_name"),
                a("action"),
                a("summary"),
                a("details"),
            ])
            .values([
                now.into(),
                AUTHOR.into(),
                "masters.migrated".into(),
                summary.into(),
                details.to_string().into(),
            ])
            .map_err(|e| DbErr::Migration(format!("запись журнала: {e}")))?;
        db.execute(&insert).await?;
        Ok(())
    }
}
