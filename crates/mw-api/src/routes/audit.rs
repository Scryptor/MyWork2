//! Журнал действий.

use axum::extract::State;

use crate::DEFAULT_PAGE_LIMIT;
use crate::auth::CurrentActor;
use crate::dto::{AuditListQuery, AuditPage};
use crate::error::ApiError;
use crate::extract::{Json, Query};
use crate::state::AppState;

/// Журнал, новые записи сверху. Без `order_id` и `client_id` нужно право `audit_view`;
/// история заказа или клиента доступна любому вошедшему.
#[utoipa::path(
    get,
    path = "/api/v1/audit",
    tag = "Журнал",
    security(("bearer" = [])),
    params(AuditListQuery),
    responses((status = 200, description = "Страница журнала", body = AuditPage))
)]
pub(super) async fn list_audit(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Query(q): Query<AuditListQuery>,
) -> Result<Json<AuditPage>, ApiError> {
    let limit = q.limit.unwrap_or(DEFAULT_PAGE_LIMIT);
    let offset = q.offset.unwrap_or(0);
    let page = state
        .core
        .list_audit(&actor, &q.into(), limit, offset)
        .await?;
    Ok(Json(page.into()))
}
