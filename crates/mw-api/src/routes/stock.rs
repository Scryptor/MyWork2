//! Склад: позиции.

use axum::body::Bytes;
use axum::extract::State;
use axum::extract::rejection::BytesRejection;
use axum::http::StatusCode;
use mw_core::{Attachment, NewStockItem, PageRequest, StockItem, StockItemPatch, StockSummary};

use crate::DEFAULT_PAGE_LIMIT;
use crate::auth::CurrentActor;
use crate::dto::{
    AttachmentNameQuery, AuditPage, PageQuery, StockListQuery, StockPage, VersionParam,
};
use crate::error::ApiError;
use crate::extract::{Json, Path, Query};
use crate::state::AppState;

/// Список и поиск склада (право `stock_view`).
#[utoipa::path(
    get,
    path = "/api/v1/stock",
    tag = "Склад",
    security(("bearer" = [])),
    params(StockListQuery),
    responses((status = 200, description = "Страница склада", body = StockPage))
)]
pub(super) async fn list_stock(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Query(q): Query<StockListQuery>,
) -> Result<Json<StockPage>, ApiError> {
    let page = state
        .core
        .list_stock(
            &actor,
            &q.filter(),
            q.limit.unwrap_or(DEFAULT_PAGE_LIMIT),
            q.offset.unwrap_or(0),
        )
        .await?;
    Ok(Json(page.into()))
}

/// Сводка склада: сколько позиций надо докупить (право `stock_view`).
#[utoipa::path(
    get,
    path = "/api/v1/stock/summary",
    tag = "Склад",
    security(("bearer" = [])),
    responses((status = 200, description = "Сводка склада", body = StockSummary))
)]
pub(super) async fn stock_summary(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
) -> Result<Json<StockSummary>, ApiError> {
    Ok(Json(state.core.stock_summary(&actor).await?))
}

/// Позиция склада (право `stock_view`); удалённая — 404.
#[utoipa::path(
    get,
    path = "/api/v1/stock/{id}",
    tag = "Склад",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер позиции")),
    responses((status = 200, description = "Позиция склада", body = StockItem))
)]
pub(super) async fn get_stock_item(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
) -> Result<Json<StockItem>, ApiError> {
    Ok(Json(state.core.get_stock_item(&actor, id).await?))
}

/// Новая позиция склада (право `stock_create`). Не переданное поле — пустое (ноль).
#[utoipa::path(
    post,
    path = "/api/v1/stock",
    tag = "Склад",
    security(("bearer" = [])),
    request_body = NewStockItem,
    responses((status = 201, description = "Позиция добавлена", body = StockItem))
)]
pub(super) async fn create_stock_item(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Json(new): Json<NewStockItem>,
) -> Result<(StatusCode, Json<StockItem>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(state.core.create_stock_item(&actor, new).await?),
    ))
}

/// Изменение позиции по версии записи (право `stock_edit`). Правка без различий успешна и версию
/// не проверяет.
#[utoipa::path(
    patch,
    path = "/api/v1/stock/{id}",
    tag = "Склад",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер позиции"), VersionParam),
    request_body = StockItemPatch,
    responses((status = 200, description = "Позиция после изменения", body = StockItem))
)]
pub(super) async fn update_stock_item(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
    Query(v): Query<VersionParam>,
    Json(patch): Json<StockItemPatch>,
) -> Result<Json<StockItem>, ApiError> {
    Ok(Json(
        state
            .core
            .update_stock_item(&actor, id, v.version, patch)
            .await?,
    ))
}

/// Удаление позиции по версии записи (право `stock_delete`): из списка пропадает, запчасти
/// заказов остаются.
#[utoipa::path(
    delete,
    path = "/api/v1/stock/{id}",
    tag = "Склад",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер позиции"), VersionParam),
    responses((status = 204, description = "Позиция удалена"))
)]
pub(super) async fn delete_stock_item(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
    Query(v): Query<VersionParam>,
) -> Result<StatusCode, ApiError> {
    state.core.delete_stock_item(&actor, id, v.version).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// История позиции, новые записи сверху (право `stock_view`, без права на весь журнал).
#[utoipa::path(
    get,
    path = "/api/v1/stock/{id}/history",
    tag = "Склад",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер позиции"), PageQuery),
    responses((status = 200, description = "Страница истории", body = AuditPage))
)]
pub(super) async fn stock_item_history(
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
        state
            .core
            .stock_item_history(&actor, id, page)
            .await?
            .into(),
    ))
}

/// Фото позиции по времени добавления (право `stock_view`).
#[utoipa::path(
    get,
    path = "/api/v1/stock/{id}/photos",
    tag = "Склад",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер позиции")),
    responses((status = 200, description = "Фото позиции", body = Vec<Attachment>))
)]
pub(super) async fn list_stock_photos(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
) -> Result<Json<Vec<Attachment>>, ApiError> {
    Ok(Json(state.core.list_stock_photos(&actor, id).await?))
}

/// Загрузка фото позиции (право `stock_edit`): не больше трёх, только картинки. Тело — сами
/// байты, `Content-Type: application/octet-stream`; не больше 25 МБ. Подпись и удаление — общие
/// `PATCH`/`DELETE /api/v1/attachments/{id}`.
#[utoipa::path(
    post,
    path = "/api/v1/stock/{id}/photos",
    tag = "Склад",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер позиции"), AttachmentNameQuery),
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses((status = 201, description = "Фото добавлено", body = Attachment))
)]
pub(super) async fn upload_stock_photo(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
    Query(q): Query<AttachmentNameQuery>,
    body: Result<Bytes, BytesRejection>,
) -> Result<(StatusCode, Json<Attachment>), ApiError> {
    let name = super::attachments::checked_name(&q.name)?;
    let bytes = body?.to_vec();
    Ok((
        StatusCode::CREATED,
        Json(state.core.add_stock_photo(&actor, id, &name, bytes).await?),
    ))
}
