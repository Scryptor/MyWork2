//! Выгрузки списков заказов и клиентов в Excel (право `data_export`).

use axum::extract::State;
use axum::response::Response;

use super::headers::export_response;
use crate::auth::CurrentActor;
use crate::dto::{ClientExportQuery, OrderExportQuery};
use crate::error::ApiError;
use crate::extract::Query;
use crate::state::AppState;

/// Заказы файлом Excel: фильтр и порядок как у списка заказов, без страниц (право `data_export`).
/// Больше 50 000 заказов — 422 `rows`/`too_many_rows`.
#[utoipa::path(
    get,
    path = "/api/v1/exports/orders",
    tag = "Выгрузки",
    security(("bearer" = [])),
    params(OrderExportQuery),
    responses((status = 200, description = "Файл Excel", content(
        (Vec<u8> = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
    )))
)]
pub(super) async fn export_orders(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Query(q): Query<OrderExportQuery>,
) -> Result<Response, ApiError> {
    let (filter, sort) = q.into_filter_and_sort();
    Ok(export_response(
        state.core.export_orders_xlsx(&actor, &filter, sort).await?,
    ))
}

/// Клиенты файлом Excel: поиск и порядок как у списка клиентов, без страниц (права `data_export`
/// и `clients_view`). Больше 50 000 клиентов — 422 `rows`/`too_many_rows`.
#[utoipa::path(
    get,
    path = "/api/v1/exports/clients",
    tag = "Выгрузки",
    security(("bearer" = [])),
    params(ClientExportQuery),
    responses((status = 200, description = "Файл Excel", content(
        (Vec<u8> = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
    )))
)]
pub(super) async fn export_clients(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Query(q): Query<ClientExportQuery>,
) -> Result<Response, ApiError> {
    Ok(export_response(
        state.core.export_clients_xlsx(&actor, &q.filter()).await?,
    ))
}
