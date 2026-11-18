//! Похожие клиенты и объединение клиентов.

use std::collections::HashSet;

use mw_db::entity::{client, client_phone, order};
use mw_domain::permission::Permission;
use mw_domain::phone::phone_key;
use mw_domain::search::normalize_search;
use mw_domain::validation::FieldError;
use sea_orm::ExprTrait;
use sea_orm::sea_query::{Expr, Order, Query};
use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::clients::{
    ClientListItem, ClientPhone, display_name, find_live, list_items, load_phones, search_text,
    validate_client, write_phones,
};
use crate::sql::text_sort;
use crate::{Actor, Client, Core, CoreError, NewAuditEntry, write_audit};

const ENTITY: &str = "клиент";

/// Сколько клиентов можно влить в один за раз.
pub const MAX_MERGE_SOURCES: usize = 10;

/// Номер записи и её версия, на которую рассчитывает клиент.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VersionedId {
    /// Номер записи.
    pub id: i64,
    /// Версия записи.
    pub version: i64,
}

/// Объединение клиентов: все источники вливаются в цель.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MergeClients {
    /// Клиент, который остаётся.
    pub target_id: i64,
    /// Версия клиента, который остаётся.
    pub target_version: i64,
    /// Клиенты, которые вливаются и удаляются: от 1 до 10, без повторов и без цели.
    pub sources: Vec<VersionedId>,
    /// ФИО итогового клиента.
    pub full_name: String,
    /// Адрес итогового клиента.
    pub address: String,
    /// Заметка итогового клиента.
    pub note: String,
    /// Откуда клиент узнал о сервисе.
    pub source: String,
    /// Итоговый клиент в чёрном списке.
    pub blacklisted: bool,
}

impl Core {
    /// Клиенты, похожие на этого: то же непустое имя (без учёта регистра и ё) или общий телефон.
    pub async fn similar_clients(
        &self,
        actor: &Actor,
        id: i64,
    ) -> Result<Vec<ClientListItem>, CoreError> {
        actor.require_any(&[Permission::ClientsView, Permission::OrdersCreate])?;
        let subject = find_live(&self.db, id)
            .await?
            .ok_or(CoreError::NotFound { entity: ENTITY, id })?;
        let own_keys: Vec<String> = load_phones(&self.db, &[id])
            .await?
            .remove(&id)
            .unwrap_or_default()
            .iter()
            .map(|p| phone_key(&p.phone))
            .filter(|k| k.len() >= 6)
            .collect();
        let mut cond = Condition::any();
        if !subject.name_key.is_empty() {
            cond = cond.add(client::Column::NameKey.eq(subject.name_key.clone()));
        }
        if !own_keys.is_empty() {
            let shared = Query::select()
                .column(client_phone::Column::ClientId)
                .from(client_phone::Entity)
                .and_where(Expr::col(client_phone::Column::PhoneKey).is_in(own_keys))
                .to_owned();
            cond = cond.add(client::Column::Id.in_subquery(shared));
        }
        if cond.is_empty() {
            return Ok(Vec::new());
        }
        let models = client::Entity::find()
            .filter(client::Column::DeletedAt.is_null())
            .filter(client::Column::Id.ne(id))
            .filter(cond)
            .order_by(
                text_sort(
                    self.db.get_database_backend(),
                    client::Column::NameKey.into_expr(),
                ),
                Order::Asc,
            )
            .order_by_asc(client::Column::Id)
            .all(&self.db)
            .await?;
        list_items(&self.db, models).await
    }

    /// Вливает клиентов-источников в цель одной транзакцией (право `clients_merge`).
    pub async fn merge_clients(&self, actor: &Actor, m: MergeClients) -> Result<Client, CoreError> {
        actor.require(Permission::ClientsMerge)?;
        let source_ids: Vec<i64> = m.sources.iter().map(|s| s.id).collect();
        let distinct: HashSet<i64> = source_ids.iter().copied().collect();
        if m.sources.is_empty()
            || m.sources.len() > MAX_MERGE_SOURCES
            || distinct.len() != source_ids.len()
            || distinct.contains(&m.target_id)
        {
            return Err(CoreError::Validation(vec![FieldError::new(
                "sources", "invalid",
            )]));
        }

        let tx = self.begin_write().await?;
        let target = find_live(&tx, m.target_id)
            .await?
            .ok_or(CoreError::NotFound {
                entity: ENTITY,
                id: m.target_id,
            })?;
        if target.version != m.target_version {
            return Err(CoreError::Conflict {
                entity: ENTITY,
                id: target.id,
                current_version: target.version,
            });
        }
        let mut all_ids = vec![target.id];
        all_ids.extend(&source_ids);
        let mut phone_map = load_phones(&tx, &all_ids).await?;

        let mut snapshots: Vec<Client> = Vec::new();
        for s in &m.sources {
            let model = find_live(&tx, s.id).await?.ok_or(CoreError::NotFound {
                entity: ENTITY,
                id: s.id,
            })?;
            if model.version != s.version {
                return Err(CoreError::Conflict {
                    entity: ENTITY,
                    id: s.id,
                    current_version: model.version,
                });
            }
            let phones = phone_map.remove(&s.id).unwrap_or_default();
            snapshots.push(Client::from_parts(model, phones));
        }

        // Телефоны цели, затем источников по порядку; номера с одинаковым ключом не повторяются.
        let mut seen = HashSet::new();
        let mut merged_phones: Vec<ClientPhone> = Vec::new();
        let target_phones = phone_map.remove(&target.id).unwrap_or_default();
        for p in target_phones
            .iter()
            .chain(snapshots.iter().flat_map(|c| c.phones.iter()))
        {
            if seen.insert(phone_key(&p.phone)) {
                merged_phones.push(p.clone());
            }
        }
        let full_name = m.full_name.trim().to_owned();
        let phones = validate_client(&full_name, &merged_phones).map_err(CoreError::Validation)?;

        let now = self.now();
        let main = phones.first().map(|p| p.phone.clone()).unwrap_or_default();
        let res = client::Entity::update_many()
            .col_expr(client::Column::FullName, Expr::value(full_name.clone()))
            .col_expr(client::Column::Phone, Expr::value(main.clone()))
            .col_expr(client::Column::PhoneKey, Expr::value(phone_key(&main)))
            .col_expr(client::Column::Address, Expr::value(m.address.trim()))
            .col_expr(client::Column::Note, Expr::value(m.note.trim()))
            .col_expr(client::Column::Source, Expr::value(m.source.trim()))
            .col_expr(client::Column::Blacklisted, Expr::value(m.blacklisted))
            .col_expr(
                client::Column::SearchText,
                Expr::value(search_text(&full_name, &phones)),
            )
            .col_expr(
                client::Column::NameKey,
                Expr::value(normalize_search(&full_name)),
            )
            .col_expr(client::Column::UpdatedAt, Expr::value(now))
            .col_expr(
                client::Column::Version,
                Expr::col(client::Column::Version).add(1),
            )
            .filter(client::Column::Id.eq(target.id))
            .filter(client::Column::Version.eq(m.target_version))
            .filter(client::Column::DeletedAt.is_null())
            .exec(&tx)
            .await?;
        if res.rows_affected == 0 {
            return Err(CoreError::Conflict {
                entity: ENTITY,
                id: target.id,
                current_version: target.version,
            });
        }
        write_phones(&tx, target.id, &phones).await?;

        let moved = order::Entity::update_many()
            .col_expr(order::Column::ClientId, Expr::value(target.id))
            .filter(order::Column::ClientId.is_in(source_ids.clone()))
            .exec(&tx)
            .await?
            .rows_affected;

        // Источники скрываются (мягкое удаление), номера освобождаются; снимок остаётся в журнале.
        client_phone::Entity::delete_many()
            .filter(client_phone::Column::ClientId.is_in(source_ids.clone()))
            .exec(&tx)
            .await?;
        client::Entity::update_many()
            .col_expr(client::Column::DeletedAt, Expr::value(now))
            .col_expr(client::Column::NameKey, Expr::value(""))
            .col_expr(client::Column::Phone, Expr::value(""))
            .col_expr(client::Column::PhoneKey, Expr::value(""))
            .col_expr(
                client::Column::Version,
                Expr::col(client::Column::Version).add(1),
            )
            .filter(client::Column::Id.is_in(source_ids))
            .exec(&tx)
            .await?;

        let names: Vec<String> = snapshots
            .iter()
            .map(|c| format!("«{}»", display_name(&c.full_name, &c.phones)))
            .collect();
        write_audit(
            &tx,
            now,
            actor,
            NewAuditEntry {
                action: "client.merged",
                order_id: None,
                client_id: Some(target.id),
                summary: format!("Объединены клиенты: {}", names.join(", ")),
                details: Some(json!({ "sources": snapshots, "moved_orders": moved })),
            },
        )
        .await?;
        let updated = find_live(&tx, target.id)
            .await?
            .ok_or(CoreError::NotFound {
                entity: ENTITY,
                id: target.id,
            })?;
        tx.commit().await?;
        Ok(Client::from_parts(updated, phones))
    }
}
