use chrono::NaiveDateTime;
use mw_db::entity::audit_log;
use mw_domain::permission::Permission;
use sea_orm::ExprTrait;
use sea_orm::sea_query::{Expr, LikeExpr};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, NotSet, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};

use crate::page::validate_limit;
use crate::{Actor, Core, CoreError, Page};

/// Запись журнала для чтения. `details` — разобранный JSON; `None`, если его нет или он повреждён.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: i64,
    pub at: NaiveDateTime,
    pub user_id: Option<i64>,
    pub user_name: String,
    pub action: String,
    pub order_id: Option<i64>,
    pub client_id: Option<i64>,
    pub summary: String,
    pub details: Option<serde_json::Value>,
}

/// Условия отбора журнала. `from` и `to` включительны; `action_prefix` — буквальное начало кода.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AuditFilter {
    pub order_id: Option<i64>,
    pub client_id: Option<i64>,
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
            summary: m.summary,
            details,
        }
    }
}

/// Шаблон `LIKE` «начинается с»: `\`, `%`, `_` экранируются обратной косой.
fn like_prefix(prefix: &str) -> String {
    let mut out = String::with_capacity(prefix.len() + 1);
    for c in prefix.chars() {
        if matches!(c, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('%');
    out
}

impl Core {
    /// Журнал: новые сверху. С `order_id` или `client_id` доступен любому (история в карточке),
    /// иначе нужно право `audit_view`.
    pub async fn list_audit(
        &self,
        actor: &Actor,
        filter: &AuditFilter,
        limit: u64,
        offset: u64,
    ) -> Result<Page<AuditEntry>, CoreError> {
        if filter.order_id.is_none() && filter.client_id.is_none() {
            actor.require(Permission::AuditView)?;
        }
        validate_limit(limit)?;
        let mut select = audit_log::Entity::find();
        if let Some(id) = filter.order_id {
            select = select.filter(audit_log::Column::OrderId.eq(id));
        }
        if let Some(id) = filter.client_id {
            select = select.filter(audit_log::Column::ClientId.eq(id));
        }
        if let Some(id) = filter.user_id {
            select = select.filter(audit_log::Column::UserId.eq(id));
        }
        if let Some(prefix) = filter.action_prefix.as_deref().filter(|p| !p.is_empty()) {
            select = select.filter(
                Expr::col(audit_log::Column::Action)
                    .like(LikeExpr::new(like_prefix(prefix)).escape('\\')),
            );
        }
        if let Some(from) = filter.from {
            select = select.filter(audit_log::Column::At.gte(from));
        }
        if let Some(to) = filter.to {
            select = select.filter(audit_log::Column::At.lte(to));
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
    pub summary: String,
    pub details: Option<serde_json::Value>,
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
    }
    .insert(conn)
    .await?;
    Ok(())
}
