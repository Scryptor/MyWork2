//! Отчёты: отчёт по выданным заказам и его файл Excel.

use axum::extract::State;
use axum::response::Response;
use mw_core::OrderReport;

use super::headers::export_response;
use crate::auth::CurrentActor;
use crate::dto::OrderReportQuery;
use crate::error::ApiError;
use crate::extract::{Json, Query};
use crate::state::AppState;

/// Отчёт по выданным заказам: итоги, группы по разрезу и шкала времени (право `reports_view`).
#[utoipa::path(
    get,
    path = "/api/v1/reports/orders",
    tag = "Отчёты",
    security(("bearer" = [])),
    params(OrderReportQuery),
    responses((status = 200, description = "Отчёт по заказам", body = OrderReport))
)]
pub(super) async fn order_report(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Query(q): Query<OrderReportQuery>,
) -> Result<Json<OrderReport>, ApiError> {
    Ok(Json(state.core.order_report(&actor, &q.filter()).await?))
}

/// Отчёт по заказам файлом Excel: листы «Итоги», разрез и «Заказы» (право `reports_view`).
/// Больше 50 000 заказов — 422 `rows`/`too_many_rows`.
#[utoipa::path(
    get,
    path = "/api/v1/reports/orders/xlsx",
    tag = "Отчёты",
    security(("bearer" = [])),
    params(OrderReportQuery),
    responses((status = 200, description = "Файл Excel", content(
        (Vec<u8> = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
    )))
)]
pub(super) async fn order_report_xlsx(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Query(q): Query<OrderReportQuery>,
) -> Result<Response, ApiError> {
    Ok(export_response(
        state.core.order_report_xlsx(&actor, &q.filter()).await?,
    ))
}
