//! Начальная настройка при первом запуске.

use axum::extract::State;
use axum::http::StatusCode;
use mw_core::SetupRequest;

use crate::auth::CurrentActor;
use crate::error::ApiError;
use crate::extract::Json;
use crate::state::AppState;

/// Начальная настройка: название фирмы, первая точка и мастера одной транзакцией. Повтор после
/// настройки (или импорта) — 409 (право `settings_manage`).
#[utoipa::path(
    post,
    path = "/api/v1/setup",
    tag = "Сервер",
    security(("bearer" = [])),
    request_body = SetupRequest,
    responses((status = 204, description = "Настроено"))
)]
pub(super) async fn run_setup(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Json(body): Json<SetupRequest>,
) -> Result<StatusCode, ApiError> {
    state.core.run_setup(&actor, body).await?;
    Ok(StatusCode::NO_CONTENT)
}
