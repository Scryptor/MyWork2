//! Маршруты API; каждый раздел добавляет свои `routes!(…)`.

use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::state::AppState;

mod auth;
mod meta;

pub(crate) fn api_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(meta::server_info))
        .routes(routes!(meta::openapi_json))
        .routes(routes!(auth::login))
        .routes(routes!(auth::logout))
        .routes(routes!(auth::me))
        .routes(routes!(auth::change_password))
}
