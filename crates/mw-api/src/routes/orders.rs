//! Заказы.

use axum::extract::State;
use axum::http::StatusCode;
use mw_core::{NewOrder, Order, OrderListItem, OrderPatch};

use crate::DEFAULT_PAGE_LIMIT;
use crate::auth::CurrentActor;
use crate::dto::{
    BarcodeQuery, NextOrderNumber, OrderListQuery, OrderPage, StatusChange, VersionParam,
};
use crate::error::ApiError;
use crate::extract::{Json, Path, Query};
use crate::state::AppState;

/// Список заказов с фильтрами, сортировкой и страницами (достаточно войти;
/// `include_deleted` требует права `orders_delete`).
#[utoipa::path(
    get,
    path = "/api/v1/orders",
    tag = "Заказы",
    security(("bearer" = [])),
    params(OrderListQuery),
    responses((status = 200, description = "Страница заказов", body = OrderPage))
)]
pub(super) async fn list_orders(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Query(q): Query<OrderListQuery>,
) -> Result<Json<OrderPage>, ApiError> {
    let limit = q.limit.unwrap_or(DEFAULT_PAGE_LIMIT);
    let offset = q.offset.unwrap_or(0);
    let (filter, sort) = q.into_filter_and_sort();
    let page = state
        .core
        .list_orders(&actor, &filter, sort, limit, offset)
        .await?;
    Ok(Json(page.into()))
}

/// Заказы по штрихкоду со сканера (достаточно войти).
#[utoipa::path(
    get,
    path = "/api/v1/orders/by-barcode",
    tag = "Заказы",
    security(("bearer" = [])),
    params(BarcodeQuery),
    responses((status = 200, description = "Найденные заказы", body = Vec<OrderListItem>))
)]
pub(super) async fn orders_by_barcode(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Query(q): Query<BarcodeQuery>,
) -> Result<Json<Vec<OrderListItem>>, ApiError> {
    Ok(Json(
        state.core.find_orders_by_barcode(&actor, &q.code).await?,
    ))
}

/// Задать номер следующего заказа (право `settings_manage`).
#[utoipa::path(
    put,
    path = "/api/v1/orders/next-number",
    tag = "Заказы",
    security(("bearer" = [])),
    request_body = NextOrderNumber,
    responses((status = 204, description = "Номер задан"))
)]
pub(super) async fn set_next_order_number(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Json(body): Json<NextOrderNumber>,
) -> Result<StatusCode, ApiError> {
    state.core.set_next_order_number(&actor, body.next).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Карточка заказа (достаточно войти).
#[utoipa::path(
    get,
    path = "/api/v1/orders/{id}",
    tag = "Заказы",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер заказа")),
    responses((status = 200, description = "Заказ", body = Order))
)]
pub(super) async fn get_order(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
) -> Result<Json<Order>, ApiError> {
    Ok(Json(state.core.get_order(&actor, id).await?))
}

/// Приём устройства: новый заказ (право `orders_create`).
#[utoipa::path(
    post,
    path = "/api/v1/orders",
    tag = "Заказы",
    security(("bearer" = [])),
    request_body = NewOrder,
    responses((status = 201, description = "Заказ создан", body = Order))
)]
pub(super) async fn create_order(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Json(new): Json<NewOrder>,
) -> Result<(StatusCode, Json<Order>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(state.core.create_order(&actor, new).await?),
    ))
}

/// Изменение заказа по версии записи. Правка без различий успешна и версию не проверяет.
#[utoipa::path(
    patch,
    path = "/api/v1/orders/{id}",
    tag = "Заказы",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер заказа"), VersionParam),
    request_body = OrderPatch,
    responses((status = 200, description = "Заказ после изменения", body = Order))
)]
pub(super) async fn update_order(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
    Query(v): Query<VersionParam>,
    Json(patch): Json<OrderPatch>,
) -> Result<Json<Order>, ApiError> {
    Ok(Json(
        state
            .core
            .update_order(&actor, id, v.version, patch)
            .await?,
    ))
}

/// Смена статуса заказа по версии записи.
#[utoipa::path(
    post,
    path = "/api/v1/orders/{id}/status",
    tag = "Заказы",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер заказа"), VersionParam),
    request_body = StatusChange,
    responses((status = 200, description = "Заказ после смены статуса", body = Order))
)]
pub(super) async fn change_order_status(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
    Query(v): Query<VersionParam>,
    Json(body): Json<StatusChange>,
) -> Result<Json<Order>, ApiError> {
    Ok(Json(
        state
            .core
            .change_status(&actor, id, v.version, body.status_id)
            .await?,
    ))
}

/// Выдача заказа клиенту по версии записи; тело пустое.
#[utoipa::path(
    post,
    path = "/api/v1/orders/{id}/issue",
    tag = "Заказы",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер заказа"), VersionParam),
    responses((status = 200, description = "Заказ после выдачи", body = Order))
)]
pub(super) async fn issue_order(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
    Query(v): Query<VersionParam>,
) -> Result<Json<Order>, ApiError> {
    Ok(Json(state.core.issue_order(&actor, id, v.version).await?))
}

/// Удаление заказа по версии записи (право `orders_delete`).
#[utoipa::path(
    delete,
    path = "/api/v1/orders/{id}",
    tag = "Заказы",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер заказа"), VersionParam),
    responses((status = 204, description = "Заказ удалён"))
)]
pub(super) async fn delete_order(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
    Query(v): Query<VersionParam>,
) -> Result<StatusCode, ApiError> {
    state.core.delete_order(&actor, id, v.version).await?;
    Ok(StatusCode::NO_CONTENT)
}
