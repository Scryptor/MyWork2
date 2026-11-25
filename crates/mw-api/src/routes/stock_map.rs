//! Карта склада: шкафы и где лежат позиции.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Response;
use mw_core::documents::{CellStickerRequest, DocumentBytes};
use mw_core::{
    NewStockRack, StockCellRef, StockItem, StockMove, StockPut, StockRack, StockRackPatch,
};

use crate::auth::CurrentActor;
use crate::dto::{DocumentPages, DocumentQuery, ReorderRequest, StockRackQuery};
use crate::error::ApiError;
use crate::extract::{Json, Path, Query};
use crate::state::AppState;

use super::documents::{ParsedQuery, file_base, parse_query, pdf_response, png_response};
use super::headers::no_store;

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

/// Положить штуки из «Не разложено» в ячейку (право `stock_edit`); больше, чем не разложено, — 422.
#[utoipa::path(
    post,
    path = "/api/v1/stock/{id}/places",
    tag = "Склад",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер позиции")),
    request_body = StockPut,
    responses((status = 200, description = "Позиция после раскладки", body = StockItem))
)]
pub(super) async fn put_stock(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
    Json(put): Json<StockPut>,
) -> Result<Json<StockItem>, ApiError> {
    Ok(Json(state.core.put_stock(&actor, id, put).await?))
}

/// Переложить штуки из одной ячейки в другую (право `stock_edit`).
#[utoipa::path(
    post,
    path = "/api/v1/stock/{id}/places/move",
    tag = "Склад",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер позиции")),
    request_body = StockMove,
    responses((status = 200, description = "Позиция после перекладки", body = StockItem))
)]
pub(super) async fn move_stock(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
    Json(mv): Json<StockMove>,
) -> Result<Json<StockItem>, ApiError> {
    Ok(Json(state.core.move_stock(&actor, id, mv).await?))
}

/// Убрать всё из ячейки в «Не разложено» (право `stock_edit`).
#[utoipa::path(
    delete,
    path = "/api/v1/stock/{id}/places/{rack_id}/{shelf}/{cell}",
    tag = "Склад",
    security(("bearer" = [])),
    params(
        ("id" = i64, Path, description = "Номер позиции"),
        ("rack_id" = i64, Path, description = "Номер шкафа"),
        ("shelf" = i32, Path, description = "Номер полки"),
        ("cell" = i32, Path, description = "Номер ячейки на полке"),
    ),
    responses((status = 200, description = "Позиция после снятия с ячейки", body = StockItem))
)]
pub(super) async fn unplace_stock(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path((id, rack_id, shelf, cell)): Path<(i64, i64, i32, i32)>,
) -> Result<Json<StockItem>, ApiError> {
    let at = StockCellRef {
        rack_id,
        shelf,
        cell,
    };
    Ok(Json(state.core.unplace_stock(&actor, id, at).await?))
}

/// Наклейки шкафа: сначала шкафа, затем всех его ячеек (право `stock_view`); у шкафа 1×1 наклейка
/// одна. PDF или, для `format=png`, страницы.
#[utoipa::path(
    get,
    path = "/api/v1/stock/racks/{id}/stickers",
    tag = "Склад",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер шкафа"), DocumentQuery),
    responses((status = 200, description = "PDF или, для `format=png`, страницы PNG", content(
        (Vec<u8> = "application/pdf"),
        (DocumentPages = "application/json")
    )))
)]
pub(super) async fn rack_stickers(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
    Query(q): Query<DocumentQuery>,
) -> Result<Response, ApiError> {
    sticker_response(&state, &actor, id, None, q, "naklejki-shkafa").await
}

/// Наклейка одной ячейки (право `stock_view`). Нет шкафа — 404, ячейки нет в сетке — 422.
#[utoipa::path(
    get,
    path = "/api/v1/stock/racks/{id}/cells/{shelf}/{cell}/sticker",
    tag = "Склад",
    security(("bearer" = [])),
    params(
        ("id" = i64, Path, description = "Номер шкафа"),
        ("shelf" = i32, Path, description = "Номер полки"),
        ("cell" = i32, Path, description = "Номер ячейки на полке"),
        DocumentQuery
    ),
    responses((status = 200, description = "PDF или, для `format=png`, страницы PNG", content(
        (Vec<u8> = "application/pdf"),
        (DocumentPages = "application/json")
    )))
)]
pub(super) async fn cell_sticker(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path((id, shelf, cell)): Path<(i64, i32, i32)>,
    Query(q): Query<DocumentQuery>,
) -> Result<Response, ApiError> {
    let base = file_base(mw_domain::documents::DocumentKind::CellSticker);
    sticker_response(&state, &actor, id, Some((shelf, cell)), q, base).await
}

async fn sticker_response(
    state: &AppState,
    actor: &mw_core::Actor,
    rack_id: i64,
    cell: Option<(i32, i32)>,
    q: DocumentQuery,
    base: &str,
) -> Result<Response, ApiError> {
    let ParsedQuery {
        options,
        pages,
        output,
        raster_dpi,
    } = parse_query(&q)?;
    let request = CellStickerRequest {
        rack_id,
        cell,
        options,
        pages,
    };
    let file = state
        .core
        .export_cell_stickers(actor, request, output)
        .await?;
    let response = match file.bytes {
        DocumentBytes::Pdf(pdf) => {
            let code = state.core.cell_sticker_code(actor, rack_id, cell).await?;
            pdf_response(pdf, &format!("{base}-{code}.pdf"))
        }
        DocumentBytes::Png(pages) => png_response(&pages, file.page_size_mm, raster_dpi),
    };
    Ok(no_store(response))
}
