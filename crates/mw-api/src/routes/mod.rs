//! Маршруты API; каждый раздел добавляет свои `routes!(…)`.

use axum::extract::DefaultBodyLimit;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::state::AppState;

mod attachments;
mod audit;
mod auth;
mod catalogs;
mod clients;
mod document_templates;
mod documents;
mod legacy_import;
mod logo;
mod meta;
mod orders;
mod setup;
mod users;

pub(crate) fn api_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .merge(
            OpenApiRouter::new()
                .routes(routes!(
                    attachments::list_order_attachments,
                    attachments::upload_order_attachment
                ))
                .routes(routes!(
                    attachments::list_draft_attachments,
                    attachments::upload_draft_attachment
                ))
                .layer(DefaultBodyLimit::max(attachments::UPLOAD_BODY_LIMIT)),
        )
        .merge(
            OpenApiRouter::new()
                .routes(routes!(logo::put_logo))
                .layer(DefaultBodyLimit::max(logo::LOGO_BODY_LIMIT)),
        )
        .routes(routes!(logo::get_logo, logo::delete_logo))
        .routes(routes!(logo::get_logo_info))
        .routes(routes!(attachments::discard_draft))
        .routes(routes!(attachments::attachment_file))
        .routes(routes!(attachments::attachment_thumb))
        .routes(routes!(
            attachments::update_attachment,
            attachments::delete_attachment
        ))
        .routes(routes!(meta::server_info))
        .routes(routes!(meta::openapi_json))
        .routes(routes!(meta::about))
        .routes(routes!(setup::run_setup))
        .routes(routes!(auth::login))
        .routes(routes!(auth::logout))
        .routes(routes!(auth::me))
        .routes(routes!(auth::change_password))
        .routes(routes!(catalogs::get_settings, catalogs::update_settings))
        .routes(routes!(catalogs::list_statuses, catalogs::create_status))
        .routes(routes!(catalogs::update_status))
        .routes(routes!(catalogs::reorder_statuses))
        .routes(routes!(catalogs::status_usage))
        .routes(routes!(catalogs::dictionary_usage))
        .routes(routes!(catalogs::service_point_usage))
        .routes(routes!(
            catalogs::list_dictionary,
            catalogs::add_dictionary_item
        ))
        .routes(routes!(catalogs::reorder_dictionary))
        .routes(routes!(catalogs::merge_dictionary_items))
        .routes(routes!(catalogs::rename_dictionary_item))
        .routes(routes!(
            catalogs::update_dictionary_item,
            catalogs::delete_dictionary_item
        ))
        .routes(routes!(
            catalogs::list_service_points,
            catalogs::create_service_point
        ))
        .routes(routes!(catalogs::update_service_point))
        .routes(routes!(catalogs::reorder_service_points))
        .routes(routes!(clients::list_clients, clients::create_client))
        .routes(routes!(clients::clients_by_phone))
        .routes(routes!(clients::merge_clients))
        .routes(routes!(clients::similar_clients))
        .routes(routes!(clients::get_client, clients::update_client))
        .routes(routes!(orders::list_orders, orders::create_order))
        .routes(routes!(orders::order_summary))
        .routes(routes!(orders::order_values))
        .routes(routes!(orders::orders_by_barcode))
        .routes(routes!(
            orders::set_next_order_number,
            orders::get_next_order_number
        ))
        .routes(routes!(
            orders::get_order,
            orders::update_order,
            orders::delete_order
        ))
        .routes(routes!(orders::change_order_status))
        .routes(routes!(orders::issue_order))
        .routes(routes!(orders::order_history))
        .routes(routes!(orders::order_comments, orders::add_order_comment))
        .routes(routes!(orders::accept_warranty_case))
        .routes(routes!(documents::order_document))
        .routes(routes!(documents::sample_document))
        .routes(routes!(document_templates::list_templates))
        .routes(routes!(
            document_templates::get_template,
            document_templates::save_template
        ))
        .routes(routes!(document_templates::preview_template))
        .routes(routes!(
            document_templates::export_template_file,
            document_templates::import_template_file
        ))
        .routes(routes!(document_templates::list_fields))
        .routes(routes!(users::list_users, users::create_user))
        .routes(routes!(users::update_user))
        .routes(routes!(users::set_user_password))
        .routes(routes!(users::list_roles, users::create_role))
        .routes(routes!(users::update_role, users::delete_role))
        .routes(routes!(users::list_masters))
        .routes(routes!(audit::list_audit))
        .routes(routes!(legacy_import::check_legacy_folder))
        .routes(routes!(legacy_import::start_legacy_import))
        .routes(routes!(legacy_import::current_legacy_import))
        .routes(routes!(legacy_import::list_legacy_imports))
        .routes(routes!(legacy_import::get_legacy_import))
}
