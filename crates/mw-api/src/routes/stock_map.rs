//! Карта склада: шкафы.

use axum::extract::State;
use axum::http::StatusCode;
use mw_core::{NewStockRack, StockRack, StockRackPatch};

use crate::auth::CurrentActor;
use crate::dto::{ReorderRequest, StockRackQuery};
use crate::error::ApiError;
use crate::extract::{Json, Path, Query};
use crate::state::AppState;

/// Шкафы с картой ячеек (право `stock_view`); ячейки с подходящей позицией помечены `found`.
#[utoipa::path(
    get,
    path = "/api/v1/stock/racks",
    tag = "Склад",
    security(("bearer" = [])),
    params(StockRackQuery),
    responses((status = 200, description = "Шкафы по порядку", body = Vec<StockRack>))
)]
pub(super) async fn list_stock_racks(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Query(q): Query<StockRackQuery>,
) -> Result<Json<Vec<StockRack>>, ApiError> {
    Ok(Json(
        state.core.list_stock_racks(&actor, &q.filter()).await?,
    ))
}

/// Новый шкаф в конце списка (право `stock_edit`).
#[utoipa::path(
    post,
    path = "/api/v1/stock/racks",
    tag = "Склад",
    security(("bearer" = [])),
    request_body = NewStockRack,
    responses((status = 201, description = "Шкаф добавлен", body = StockRack))
)]
pub(super) async fn create_stock_rack(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Json(new): Json<NewStockRack>,
) -> Result<(StatusCode, Json<StockRack>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(state.core.create_stock_rack(&actor, new).await?),
    ))
}

/// Новый порядок шкафов: нужны номера всех шкафов (право `stock_edit`).
#[utoipa::path(
    put,
    path = "/api/v1/stock/racks/order",
    tag = "Склад",
    security(("bearer" = [])),
    request_body = ReorderRequest,
    responses((status = 204, description = "Порядок сохранён"))
)]
pub(super) async fn reorder_stock_racks(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Json(req): Json<ReorderRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .core
        .reorder_stock_racks(&actor, &req.ordered_ids, req.seen_ids.as_deref())
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Правка шкафа (право `stock_edit`). Ячейки, которых нет в новой сетке, должны быть пусты, иначе 409.
#[utoipa::path(
    patch,
    path = "/api/v1/stock/racks/{id}",
    tag = "Склад",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер шкафа")),
    request_body = StockRackPatch,
    responses((status = 200, description = "Шкаф после изменения", body = StockRack))
)]
pub(super) async fn update_stock_rack(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
    Json(patch): Json<StockRackPatch>,
) -> Result<Json<StockRack>, ApiError> {
    Ok(Json(state.core.update_stock_rack(&actor, id, patch).await?))
}

/// Удаление пустого шкафа насовсем (право `stock_edit`); с запчастями в ячейках — 409.
#[utoipa::path(
    delete,
    path = "/api/v1/stock/racks/{id}",
    tag = "Склад",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер шкафа")),
    responses((status = 204, description = "Шкаф удалён"))
)]
pub(super) async fn delete_stock_rack(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    state.core.delete_stock_rack(&actor, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
