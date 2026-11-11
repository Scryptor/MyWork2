use chrono::NaiveDateTime;
use mw_db::entity::audit_log;
use sea_orm::{ActiveModelTrait, ConnectionTrait, NotSet, Set};

use crate::{Actor, CoreError};

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
