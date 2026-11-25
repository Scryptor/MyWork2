//! Три настройки ссылок (`firm_telegram`, `firm_google_maps`, `firm_yandex_maps`) → список
//! `firm_links`: четыре встроенные ссылки, а старый адрес Google Карт, если был, — своей ссылкой в
//! конце. Если все три пусты, список не пишется (действует умолчание). Старые ключи уходят всегда.

use sea_orm_migration::prelude::*;
use serde_json::{Value, json};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261125_000001_firm_links"
    }
}

const OLD_KEYS: [&str; 3] = ["firm_telegram", "firm_google_maps", "firm_yandex_maps"];
const NEW_KEY: &str = "firm_links";

/// Значение старой настройки: JSON-строка без пробелов по краям; нечитаемое — пусто.
fn old_url(old_json: Option<&str>) -> String {
    old_json
        .and_then(|v| serde_json::from_str::<Value>(v).ok())
        .and_then(|v| v.as_str().map(|s| s.trim().to_owned()))
        .unwrap_or_default()
}

fn link(id: &str, name: &str, url: &str) -> Value {
    json!({"id": id, "name": name, "url": url, "enabled": true})
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        let mut select = Query::select();
        select
            .columns([Alias::new("key"), Alias::new("value")])
            .from(Alias::new("settings"))
            .and_where(
                Expr::col(Alias::new("key")).is_in(OLD_KEYS.iter().copied().chain([NEW_KEY])),
            );
        let rows = db.query_all(&select).await?;
        let mut values = std::collections::BTreeMap::new();
        for row in &rows {
            let key: String = row.try_get_by_index(0)?;
            let value: String = row.try_get_by_index(1)?;
            values.insert(key, value);
        }
        let url = |key: &str| old_url(values.get(key).map(String::as_str));
        let (telegram, google, yandex) = (
            url("firm_telegram"),
            url("firm_google_maps"),
            url("firm_yandex_maps"),
        );

        let any_old = !telegram.is_empty() || !google.is_empty() || !yandex.is_empty();
        if any_old && !values.contains_key(NEW_KEY) {
            let mut list = vec![
                link("yandex_maps", "Яндекс Карты", &yandex),
                link("2gis", "2ГИС", ""),
                link("telegram", "Telegram", &telegram),
                link("max", "MAX", ""),
            ];
            if !google.is_empty() {
                list.push(link("google_maps", "Google Карты", &google));
            }
            let mut insert = Query::insert();
            insert
                .into_table(Alias::new("settings"))
                .columns([Alias::new("key"), Alias::new("value")])
                .values([NEW_KEY.into(), Value::Array(list).to_string().into()])
                .map_err(|e| DbErr::Migration(format!("настройка ссылок: {e}")))?;
            m.exec_stmt(insert).await?;
        }

        let mut delete = Query::delete();
        delete
            .from_table(Alias::new("settings"))
            .and_where(Expr::col(Alias::new("key")).is_in(OLD_KEYS));
        m.exec_stmt(delete).await
    }
}
