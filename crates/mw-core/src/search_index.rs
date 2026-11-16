//! Единственный дом построителей `search_text` и переиндексации.
//!
//! Любая правка построителей или `mw_domain::search::normalize_search` обязана увеличить
//! `SEARCH_INDEX_VERSION`: при открытии `Core` устаревший индекс пересчитывается сам.

use std::collections::HashMap;

use mw_db::entity::{client, client_phone, order, setting};
use mw_domain::permission::Permission;
use mw_domain::search::{index_text, normalize_search};
use sea_orm::sea_query::Expr;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder};
use serde_json::{Value, json};

use crate::core::begin_write_on;
use crate::settings::put;
use crate::{Actor, Core, CoreError, NewAuditEntry, write_audit};

/// Версия правил построения индекса: 1 — планы 1–2 (без NFC, один телефон клиента),
/// 2 — NFC и все телефоны клиента, 3 — ключи сортировки и фильтров заказа.
pub const SEARCH_INDEX_VERSION: i64 = 3;

/// Ключ настройки с версией сохранённого индекса (не входит в `AppSettings`).
const VERSION_KEY: &str = "search_index_version";

/// Сколько строк пришлось переписать при переиндексации.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReindexStats {
    pub clients: u64,
    pub orders: u64,
}

/// Текст для поиска по клиенту: ФИО и все номера.
pub(crate) fn client_search_text(full_name: &str, phones: &[String]) -> String {
    let mut parts: Vec<&str> = vec![full_name];
    parts.extend(phones.iter().map(String::as_str));
    index_text(&parts)
}

/// Текст для поиска по заказу (без данных клиента: поиск соединяет таблицы).
pub(crate) fn order_search_text(m: &order::Model) -> String {
    index_text(&[
        &m.id.to_string(),
        &m.device_type,
        &m.brand,
        &m.model,
        &m.serial,
        &m.barcode,
        &m.malfunction,
        &m.color,
    ])
}

/// Заполняет ключи сортировки и фильтров заказа (нормализованные тип, бренд, модель, мастер).
pub(crate) fn set_order_keys(m: &mut order::Model) {
    m.device_type_key = normalize_search(&m.device_type);
    m.brand_key = normalize_search(&m.brand);
    m.model_key = normalize_search(&m.model);
    m.master_key = normalize_search(&m.master);
}

/// Пересчитывает `search_text` всех клиентов и заказов, пишет только отличающиеся строки.
pub(crate) async fn reindex_all(conn: &impl ConnectionTrait) -> Result<ReindexStats, CoreError> {
    let mut phones: HashMap<i64, Vec<String>> = HashMap::new();
    let rows = client_phone::Entity::find()
        .order_by_asc(client_phone::Column::ClientId)
        .order_by_asc(client_phone::Column::Position)
        .all(conn)
        .await?;
    for r in rows {
        phones.entry(r.client_id).or_default().push(r.phone);
    }

    let mut stats = ReindexStats {
        clients: 0,
        orders: 0,
    };
    for c in client::Entity::find().all(conn).await? {
        let list = phones.get(&c.id).map(Vec::as_slice).unwrap_or(&[]);
        let text = client_search_text(&c.full_name, list);
        if text != c.search_text {
            client::Entity::update_many()
                .col_expr(client::Column::SearchText, Expr::value(text))
                .filter(client::Column::Id.eq(c.id))
                .exec(conn)
                .await?;
            stats.clients += 1;
        }
    }
    for o in order::Entity::find().all(conn).await? {
        let mut fresh = o.clone();
        fresh.search_text = order_search_text(&o);
        set_order_keys(&mut fresh);
        if fresh != o {
            order::Entity::update_many()
                .col_expr(order::Column::SearchText, Expr::value(fresh.search_text))
                .col_expr(
                    order::Column::DeviceTypeKey,
                    Expr::value(fresh.device_type_key),
                )
                .col_expr(order::Column::BrandKey, Expr::value(fresh.brand_key))
                .col_expr(order::Column::ModelKey, Expr::value(fresh.model_key))
                .col_expr(order::Column::MasterKey, Expr::value(fresh.master_key))
                .filter(order::Column::Id.eq(o.id))
                .exec(conn)
                .await?;
            stats.orders += 1;
        }
    }
    Ok(stats)
}

/// Вызывается из `Core::open`: если версия индекса в базе другая, пересчитывает его и
/// записывает версию в одной транзакции. Журнал не пишется.
pub(crate) async fn ensure_index(db: &sea_orm::DatabaseConnection) -> Result<(), CoreError> {
    let tx = begin_write_on(db).await?;
    let stored = setting::Entity::find_by_id(VERSION_KEY)
        .one(&tx)
        .await?
        .and_then(|r| serde_json::from_str::<Value>(&r.value).ok())
        .and_then(|v| v.as_i64());
    if stored != Some(SEARCH_INDEX_VERSION) {
        reindex_all(&tx).await?;
        put(&tx, VERSION_KEY, &json!(SEARCH_INDEX_VERSION)).await?;
    }
    tx.commit().await?;
    Ok(())
}

impl Core {
    /// Ручная переиндексация поиска.
    pub async fn reindex_search(&self, actor: &Actor) -> Result<ReindexStats, CoreError> {
        actor.require(Permission::SettingsManage)?;
        let tx = self.begin_write().await?;
        let stats = reindex_all(&tx).await?;
        put(&tx, VERSION_KEY, &json!(SEARCH_INDEX_VERSION)).await?;
        write_audit(
            &tx,
            self.now(),
            actor,
            NewAuditEntry {
                action: "search.reindexed",
                order_id: None,
                client_id: None,
                summary: format!(
                    "Поиск переиндексирован: клиентов {}, заказов {}",
                    stats.clients, stats.orders
                ),
                details: Some(json!({ "clients": stats.clients, "orders": stats.orders })),
            },
        )
        .await?;
        tx.commit().await?;
        Ok(stats)
    }
}
