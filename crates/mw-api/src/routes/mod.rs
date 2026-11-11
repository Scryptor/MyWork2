//! Маршруты API; каждый раздел добавляет свои `routes!(…)`.

use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::state::AppState;

mod auth;
mod catalogs;
mod meta;

pub(crate) fn api_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(meta::server_info))
        .routes(routes!(meta::openapi_json))
        .routes(routes!(auth::login))
        .routes(routes!(auth::logout))
        .routes(routes!(auth::me))
        .routes(routes!(auth::change_password))
        .routes(routes!(catalogs::get_settings, catalogs::update_settings))
        .routes(routes!(catalogs::list_statuses, catalogs::create_status))
        .routes(routes!(catalogs::update_status))
        .routes(routes!(catalogs::reorder_statuses))
        .routes(routes!(
            catalogs::list_dictionary,
            catalogs::add_dictionary_item
        ))
        .routes(routes!(catalogs::reorder_dictionary))
        .routes(routes!(
            catalogs::rename_dictionary_item,
            catalogs::delete_dictionary_item
        ))
        .routes(routes!(
            catalogs::list_service_points,
            catalogs::create_service_point
        ))
        .routes(routes!(catalogs::update_service_point))
        .routes(routes!(catalogs::reorder_service_points))
}
