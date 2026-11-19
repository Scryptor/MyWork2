//! Сколько записей используют значение справочника, статус или точку обслуживания.

use std::collections::HashMap;

use mw_db::entity::dictionary_item;
use mw_domain::dictionary::DictionaryKind;
use mw_domain::permission::Permission;
use mw_domain::search::normalize_search;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Statement};
use serde::{Deserialize, Serialize};

use crate::{Actor, Core, CoreError};

/// Сколько неудалённых записей используют значение; значения без использования в ответ не попадают.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct CatalogUsage {
    /// Номер значения, статуса или точки обслуживания.
    pub id: i64,
    /// Сколько заказов (для источника клиента — клиентов) его используют.
    #[ts(type = "number")]
    pub count: u64,
}

/// Таблица и поле, где хранится значение справочника.
pub(crate) struct UsageTarget {
    pub(crate) table: &'static str,
    pub(crate) column: &'static str,
    /// Поле хранит список через запятую.
    pub(crate) list: bool,
}

pub(crate) fn usage_target(kind: DictionaryKind) -> UsageTarget {
    let (table, column, list) = match kind {
        DictionaryKind::DeviceType => ("orders", "device_type", false),
        DictionaryKind::Brand => ("orders", "brand", false),
        DictionaryKind::Color => ("orders", "color", false),
        DictionaryKind::Master => ("orders", "master", false),
        DictionaryKind::Warranty => ("orders", "warranty_text", false),
        DictionaryKind::Completeness => ("orders", "completeness", true),
        DictionaryKind::Condition => ("orders", "device_condition", true),
        DictionaryKind::Malfunction => ("orders", "malfunction", true),
        DictionaryKind::WorkDone => ("orders", "work_done", true),
        DictionaryKind::ClientSource => ("clients", "source", false),
    };
    UsageTarget {
        table,
        column,
        list,
    }
}

/// `SELECT <поле>, COUNT(*) … GROUP BY <поле>` по неудалённым записям. Имена берутся только из
/// статических таблиц этого модуля.
async fn group_counts<T>(
    conn: &impl ConnectionTrait,
    table: &str,
    column: &str,
    read: impl Fn(&sea_orm::QueryResult) -> Result<Option<T>, sea_orm::DbErr>,
) -> Result<Vec<(T, u64)>, CoreError> {
    let sql = format!(
        r#"SELECT "{column}", COUNT(*) FROM "{table}" WHERE "deleted_at" IS NULL GROUP BY "{column}""#
    );
    let rows = conn
        .query_all_raw(Statement::from_string(conn.get_database_backend(), sql))
        .await?;
    let mut out = Vec::new();
    for row in rows {
        let count = row.try_get_by_index::<i64>(1)?;
        if let Some(key) = read(&row)? {
            out.push((key, u64::try_from(count).unwrap_or(0)));
        }
    }
    Ok(out)
}

fn sorted(map: HashMap<i64, u64>) -> Vec<CatalogUsage> {
    let mut out: Vec<CatalogUsage> = map
        .into_iter()
        .filter(|(_, count)| *count > 0)
        .map(|(id, count)| CatalogUsage { id, count })
        .collect();
    out.sort_by_key(|u| u.id);
    out
}

impl Core {
    /// Сколько заказов (клиентов) используют каждое значение справочника.
    pub async fn dictionary_usage(
        &self,
        actor: &Actor,
        kind: DictionaryKind,
    ) -> Result<Vec<CatalogUsage>, CoreError> {
        actor.require(Permission::SettingsManage)?;
        let target = usage_target(kind);
        // Повторы с разным регистром (из MyWork 2) — один ключ на несколько значений: счёт у каждого.
        let mut ids_by_key: HashMap<String, Vec<i64>> = HashMap::new();
        for m in dictionary_item::Entity::find()
            .filter(dictionary_item::Column::Kind.eq(kind.as_str()))
            .all(&self.db)
            .await?
        {
            ids_by_key
                .entry(normalize_search(&m.value))
                .or_default()
                .push(m.id);
        }
        let groups = group_counts(&self.db, target.table, target.column, |row| {
            row.try_get_by_index::<Option<String>>(0)
        })
        .await?;
        let mut totals: HashMap<i64, u64> = HashMap::new();
        for (text, count) in groups {
            let mut keys: Vec<String> = if target.list {
                text.split(',').map(normalize_search).collect()
            } else {
                vec![normalize_search(&text)]
            };
            keys.retain(|k| !k.is_empty());
            keys.sort();
            keys.dedup();
            for key in keys {
                for id in ids_by_key.get(&key).into_iter().flatten() {
                    *totals.entry(*id).or_default() += count;
                }
            }
        }
        Ok(sorted(totals))
    }

    /// Сколько неудалённых заказов в каждом статусе.
    pub async fn status_usage(&self, actor: &Actor) -> Result<Vec<CatalogUsage>, CoreError> {
        actor.require(Permission::SettingsManage)?;
        self.id_usage("status_id").await
    }

    /// Сколько неудалённых заказов у каждой точки обслуживания.
    pub async fn service_point_usage(&self, actor: &Actor) -> Result<Vec<CatalogUsage>, CoreError> {
        actor.require(Permission::SettingsManage)?;
        self.id_usage("service_point_id").await
    }

    async fn id_usage(&self, column: &str) -> Result<Vec<CatalogUsage>, CoreError> {
        let groups = group_counts(&self.db, "orders", column, |row| {
            row.try_get_by_index::<Option<i64>>(0)
        })
        .await?;
        Ok(sorted(groups.into_iter().collect()))
    }
}

impl Core {
    /// Сколько неудалённых заказов и клиентов в базе (для окна «О программе»).
    pub async fn about_totals(&self, actor: &Actor) -> Result<(u64, u64), CoreError> {
        actor.require(Permission::SettingsManage)?;
        let count = |table: &'static str| async move {
            let sql = format!(r#"SELECT COUNT(*) FROM "{table}" WHERE "deleted_at" IS NULL"#);
            let row = self
                .db
                .query_one_raw(Statement::from_string(self.db.get_database_backend(), sql))
                .await?
                .ok_or_else(|| CoreError::Internal(format!("COUNT({table})")))?;
            Ok::<u64, CoreError>(u64::try_from(row.try_get_by_index::<i64>(0)?).unwrap_or(0))
        };
        Ok((count("orders").await?, count("clients").await?))
    }
}
