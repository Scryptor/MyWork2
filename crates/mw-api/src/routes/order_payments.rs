//! Платежи по заказу.

use axum::extract::State;
use mw_core::{NewOrderPayment, OrderPayments};

use crate::auth::CurrentActor;
use crate::error::ApiError;
use crate::extract::{Json, Path};
use crate::state::AppState;

/// Платежи заказа (достаточно войти; удалённый заказ читается).
#[utoipa::path(
    get,
    path = "/api/v1/orders/{id}/payments",
    tag = "Заказы",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер заказа")),
    responses((status = 200, description = "Платежи заказа", body = OrderPayments))
)]
pub(super) async fn order_payments(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
) -> Result<Json<OrderPayments>, ApiError> {
    Ok(Json(state.core.list_order_payments(&actor, id).await?))
}

/// Приём платежа (право `orders_edit`). День вне отрезка «приём заказа — сегодня» — 422, поле
/// `day`, код `payment_day`. Повтор с тем же `request_key` и тем же содержимым не добавляет
/// платёж; с другим — 422, поле `request_key`, код `duplicate`. Версия заказа не меняется.
#[utoipa::path(
    post,
    path = "/api/v1/orders/{id}/payments",
    tag = "Заказы",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер заказа")),
    request_body = NewOrderPayment,
    responses((status = 200, description = "Платежи заказа после приёма", body = OrderPayments))
)]
pub(super) async fn add_order_payment(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
    Json(new): Json<NewOrderPayment>,
) -> Result<Json<OrderPayments>, ApiError> {
    Ok(Json(state.core.add_order_payment(&actor, id, new).await?))
}

/// Удаление платежа (право `orders_edit`). Платёж другого заказа или уже удалённый — 404.
#[utoipa::path(
    delete,
    path = "/api/v1/orders/{id}/payments/{payment_id}",
    tag = "Заказы",
    security(("bearer" = [])),
    params(
        ("id" = i64, Path, description = "Номер заказа"),
        ("payment_id" = i64, Path, description = "Номер платежа")
    ),
    responses((status = 200, description = "Оставшиеся платежи заказа", body = OrderPayments))
)]
pub(super) async fn remove_order_payment(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path((id, payment_id)): Path<(i64, i64)>,
) -> Result<Json<OrderPayments>, ApiError> {
    Ok(Json(
        state
            .core
            .remove_order_payment(&actor, id, payment_id)
            .await?,
    ))
}
