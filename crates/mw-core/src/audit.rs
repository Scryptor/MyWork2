use chrono::{NaiveDateTime, SubsecRound};
use mw_db::entity::audit_log;
use mw_domain::permission::Permission;
use sea_orm::ExprTrait;
use sea_orm::sea_query::{Expr, LikeExpr};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, NotSet, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};

use crate::page::validate_page;
use crate::{Actor, Core, CoreError, Page};

/// Запись журнала для чтения. `details` — разобранный JSON; `null`, если его нет или он повреждён.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct AuditEntry {
    /// Номер записи.
    pub id: i64,
    /// Когда произошло (местное время предприятия).
    pub at: NaiveDateTime,
    /// Номер пользователя; пусто для системных действий.
    #[schema(required = true)]
    pub user_id: Option<i64>,
    /// Имя пользователя на момент действия.
    pub user_name: String,
    /// Код действия.
    pub action: String,
    /// Номер заказа, к которому относится запись.
    #[schema(required = true)]
    pub order_id: Option<i64>,
    /// Номер клиента, к которому относится запись.
    #[schema(required = true)]
    pub client_id: Option<i64>,
    /// Номер позиции склада, к которой относится запись.
    #[schema(required = true)]
    pub stock_item_id: Option<i64>,
    /// Краткое описание.
    pub summary: String,
    /// Подробности в JSON; пусто, если их нет.
    #[schema(required = true)]
    pub details: Option<serde_json::Value>,
}

/// Условия отбора журнала. `from` и `to` включительны; `action_prefix` — буквальное начало кода.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AuditFilter {
    pub order_id: Option<i64>,
    pub client_id: Option<i64>,
    pub stock_item_id: Option<i64>,
    pub user_id: Option<i64>,
    pub action_prefix: Option<String>,
    pub from: Option<NaiveDateTime>,
    pub to: Option<NaiveDateTime>,
}

impl From<audit_log::Model> for AuditEntry {
    fn from(m: audit_log::Model) -> Self {
        let details = m
            .details
            .and_then(|text| match serde_json::from_str(&text) {
                Ok(v) => Some(v),
                Err(_) => {
                    tracing::warn!(audit_id = m.id, "повреждённый JSON в деталях журнала");
                    None
                }
            });
        AuditEntry {
            id: m.id,
            at: m.at,
            user_id: m.user_id,
            user_name: m.user_name,
            action: m.action,
            order_id: m.order_id,
            client_id: m.client_id,
            stock_item_id: m.stock_item_id,
            summary: m.summary,
            details,
        }
    }
}

impl Core {
    /// Журнал: новые сверху. С `order_id` или `client_id` доступен любому (история в карточке),
    /// с `stock_item_id` — с правом `stock_view` (история позиции склада), иначе нужно право
    /// `audit_view`.
    pub async fn list_audit(
        &self,
        actor: &Actor,
        filter: &AuditFilter,
        limit: u64,
        offset: u64,
    ) -> Result<Page<AuditEntry>, CoreError> {
        if filter.order_id.is_none() && filter.client_id.is_none() {
            if filter.stock_item_id.is_some() {
                actor.require_any(&[Permission::StockView, Permission::AuditView])?;
            } else {
                actor.require(Permission::AuditView)?;
            }
        }
        validate_page(limit, offset)?;
        let mut select = audit_log::Entity::find();
        if let Some(id) = filter.order_id {
            select = select.filter(audit_log::Column::OrderId.eq(id));
        }
        if let Some(id) = filter.client_id {
            select = select.filter(audit_log::Column::ClientId.eq(id));
        }
        if let Some(id) = filter.stock_item_id {
            select = select.filter(audit_log::Column::StockItemId.eq(id));
        }
        if let Some(id) = filter.user_id {
            select = select.filter(audit_log::Column::UserId.eq(id));
        }
        if let Some(prefix) = filter.action_prefix.as_deref().filter(|p| !p.is_empty()) {
            select = select.filter(
                Expr::col(audit_log::Column::Action)
                    .like(LikeExpr::new(mw_domain::search::like_prefix(prefix)).escape('\\')),
            );
        }
        if let Some(from) = filter.from {
            select = select.filter(audit_log::Column::At.gte(from.trunc_subsecs(0)));
        }
        if let Some(to) = filter.to {
            select = select.filter(audit_log::Column::At.lte(to.trunc_subsecs(0)));
        }
        let total = select.clone().count(&self.db).await?;
        let items = select
            .order_by_desc(audit_log::Column::At)
            .order_by_desc(audit_log::Column::Id)
            .limit(limit)
            .offset(offset)
            .all(&self.db)
            .await?
            .into_iter()
            .map(AuditEntry::from)
            .collect();
        Ok(Page { items, total })
    }
}

/// Запись журнала действий. `summary` — по-русски, без паролей и других секретов.
pub(crate) struct NewAuditEntry {
    pub action: &'static str,
    pub order_id: Option<i64>,
    pub client_id: Option<i64>,
    pub stock_item_id: Option<i64>,
    pub summary: String,
    pub details: Option<serde_json::Value>,
}

/// Откуда пришла правка записи, если не из неё самой: приписка к сводке записи журнала, чтобы в
/// истории было видно, что заказ, клиента или позицию склада изменил не человек в карточке. В
/// `details` пометки нет: там лежит карта полей правки.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditVia {
    /// Переименование или объединение значения справочника.
    Dictionary,
}

impl EditVia {
    /// Приписка к сводке записи журнала.
    pub(crate) fn suffix_ru(self) -> &'static str {
        match self {
            Self::Dictionary => " (из справочника)",
        }
    }
}

/// Пишет в журнал через переданное соединение; вызывается внутри транзакции изменения.
pub(crate) async fn write_audit(
    conn: &impl ConnectionTrait,
    at: NaiveDateTime,
    actor: &Actor,
    e: NewAuditEntry,
) -> Result<(), CoreError> {
    audit_log::ActiveModel {
        id: NotSet,
        at: Set(at),
        user_id: Set(actor.user_id),
        user_name: Set(actor.name.clone()),
        action: Set(e.action.to_owned()),
        order_id: Set(e.order_id),
        client_id: Set(e.client_id),
        summary: Set(e.summary),
        details: Set(e.details.map(|d| d.to_string())),
        stock_item_id: Set(e.stock_item_id),
    }
    .insert(conn)
    .await?;
    Ok(())
}
