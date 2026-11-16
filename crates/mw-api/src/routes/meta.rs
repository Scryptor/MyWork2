//! Сведения о сервере и описание API.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::header;
use axum::response::IntoResponse;

use crate::dto::ServerInfo;
use crate::error::ApiError;
use crate::extract::Json;
use crate::state::AppState;

/// Сведения о сервере: версия и нужен ли вход. Вход не требуется.
#[utoipa::path(
    get,
    path = "/api/v1/server-info",
    tag = "Сервер",
    responses((status = 200, description = "Сведения о сервере", body = ServerInfo))
)]
pub(super) async fn server_info(
    State(state): State<AppState>,
) -> Result<Json<ServerInfo>, ApiError> {
    let open_mode = state.core.is_open_mode().await?;
    Ok(Json(ServerInfo {
        app: "mw".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        api_version: crate::API_VERSION,
        auth_required: !(state.config.loopback() && open_mode),
        has_users: !open_mode,
        server_time: state.core.now().format("%Y-%m-%dT%H:%M:%S").to_string(),
    }))
}

/// Описание API в формате OpenAPI 3.1. Вход не требуется.
#[utoipa::path(
    get,
    path = "/api/v1/openapi.json",
    tag = "Сервер",
    responses((status = 200, description = "Документ OpenAPI", content_type = "application/json", body = serde_json::Value))
)]
pub(super) async fn openapi_json(State(state): State<AppState>) -> impl IntoResponse {
    let body: Bytes = state.openapi_json.clone();
    ([(header::CONTENT_TYPE, "application/json")], body)
}
