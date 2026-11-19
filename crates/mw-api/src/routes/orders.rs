//! Заказы.

use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use mw_core::{
    NewOrder, NewWarrantyCase, Order, OrderListItem, OrderPatch, OrderSummary, PageRequest,
};

use crate::DEFAULT_PAGE_LIMIT;
use crate::auth::CurrentActor;
use crate::dto::{
    AuditPage, BarcodeQuery, NextOrderNumber, OrderListQuery, OrderPage, OrderSummaryQuery,
    OrderValuesQuery, PageQuery, StatusChange, VersionParam,
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

/// Сводка: заказы по статусам и сколько требуют внимания (достаточно войти).
#[utoipa::path(
    get,
    path = "/api/v1/orders/summary",
    tag = "Заказы",
    security(("bearer" = [])),
    params(OrderSummaryQuery),
    responses((status = 200, description = "Сводка по заказам", body = OrderSummary))
)]
pub(super) async fn order_summary(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Query(q): Query<OrderSummaryQuery>,
) -> Result<Json<OrderSummary>, ApiError> {
    Ok(Json(
        state.core.order_summary(&actor, q.service_point_id).await?,
    ))
}

/// Подсказки значений поля заказа: самые частые, не больше 20 (достаточно войти).
#[utoipa::path(
    get,
    path = "/api/v1/orders/values",
    tag = "Заказы",
    security(("bearer" = [])),
    params(OrderValuesQuery),
    responses((status = 200, description = "Значения", body = Vec<String>))
)]
pub(super) async fn order_values(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Query(q): Query<OrderValuesQuery>,
) -> Result<Json<Vec<String>>, ApiError> {
    Ok(Json(
        state
            .core
            .order_values(
                &actor,
                q.field,
                q.query.as_deref().unwrap_or_default(),
                q.brand.as_deref(),
            )
            .await?,
    ))
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

/// Номер, который получит следующий заказ (право `settings_manage`).
#[utoipa::path(
    get,
    path = "/api/v1/orders/next-number",
    tag = "Заказы",
    security(("bearer" = [])),
    responses((status = 200, description = "Следующий номер", body = NextOrderNumber))
)]
pub(super) async fn get_next_order_number(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
) -> Result<Json<NextOrderNumber>, ApiError> {
    Ok(Json(NextOrderNumber {
        next: state.core.next_order_number(&actor).await?,
    }))
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
    responses((
        status = 201,
        description = "Заказ создан. Повтор с тем же `request_key` отвечает тем же заказом \
            и заголовком `X-MW-Replayed: 1`",
        body = Order,
        headers(("X-MW-Replayed" = String, description = "`1` — заказ уже был создан раньше"))
    ))
)]
pub(super) async fn create_order(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Json(new): Json<NewOrder>,
) -> Result<(StatusCode, HeaderMap, Json<Order>), ApiError> {
    let (order, replayed) = state.core.create_order_marked(&actor, new).await?;
    let mut headers = HeaderMap::new();
    if replayed {
        headers.insert("x-mw-replayed", HeaderValue::from_static("1"));
    }
    Ok((StatusCode::CREATED, headers, Json(order)))
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
            .change_status_with(&actor, id, v.version, body.status_id, body.stamp_issued_at)
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

/// История заказа: записи журнала, новые сверху. Права на журнал не нужно, достаточно войти.
#[utoipa::path(
    get,
    path = "/api/v1/orders/{id}/history",
    tag = "Заказы",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер заказа"), PageQuery),
    responses((status = 200, description = "Страница истории заказа", body = AuditPage))
)]
pub(super) async fn order_history(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
    Query(q): Query<PageQuery>,
) -> Result<Json<AuditPage>, ApiError> {
    let page = PageRequest {
        limit: q.limit.unwrap_or(DEFAULT_PAGE_LIMIT),
        offset: q.offset.unwrap_or(0),
    };
    Ok(Json(
        state.core.order_history(&actor, id, page).await?.into(),
    ))
}

/// Приём выданного заказа по гарантии: статус «Принят по гарантии» и запись в истории.
#[utoipa::path(
    post,
    path = "/api/v1/orders/{id}/warranty-case",
    tag = "Заказы",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер заказа"), VersionParam),
    request_body = NewWarrantyCase,
    responses((status = 200, description = "Заказ после приёма по гарантии", body = Order))
)]
pub(super) async fn accept_warranty_case(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
    Query(v): Query<VersionParam>,
    Json(body): Json<NewWarrantyCase>,
) -> Result<Json<Order>, ApiError> {
    Ok(Json(
        state
            .core
            .accept_warranty_case(&actor, id, v.version, body)
            .await?,
    ))
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
