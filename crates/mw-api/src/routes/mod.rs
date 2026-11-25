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
mod headers;
mod legacy_import;
mod logo;
mod meta;
mod orders;
mod reports;
mod setup;
mod stock;
mod users;

/// Все маршруты API.
///
/// Разделы собираются отдельными функциями и сливаются по очереди: в отладочной сборке временные
/// значения `routes!(…)` не делят слоты стека, и одна функция со всеми маршрутами занимала больше
/// полумегабайта стека — на Windows (1 МБ у главного потока) сервер падал при запуске. Новые
/// маршруты — в свой раздел; размер держит тест `tests/stack.rs`.
pub(crate) fn api_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .merge(file_routes())
        .merge(meta_routes())
        .merge(catalog_routes())
        .merge(client_routes())
        .merge(stock_routes())
        .merge(order_routes())
        .merge(document_routes())
        .merge(report_routes())
        .merge(user_routes())
        .merge(legacy_import_routes())
}

/// Вложения, черновики, фото склада и логотип; загрузки — со своим пределом тела.
#[inline(never)]
fn file_routes() -> OpenApiRouter<AppState> {
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
                .routes(routes!(stock::upload_stock_photo))
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
}

/// Сведения о сервере, документ API, первичная настройка и вход.
#[inline(never)]
fn meta_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(meta::server_info))
        .routes(routes!(meta::openapi_json))
        .routes(routes!(meta::about))
        .routes(routes!(setup::run_setup))
        .routes(routes!(auth::login))
        .routes(routes!(auth::logout))
        .routes(routes!(auth::me))
        .routes(routes!(auth::change_password))
}

/// Настройки, статусы, справочники и точки приёма.
#[inline(never)]
fn catalog_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
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
}

/// Клиенты.
#[inline(never)]
fn client_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(clients::list_clients, clients::create_client))
        .routes(routes!(clients::clients_by_phone))
        .routes(routes!(clients::merge_clients))
        .routes(routes!(clients::similar_clients))
        .routes(routes!(clients::get_client, clients::update_client))
}

/// Склад.
#[inline(never)]
fn stock_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(stock::list_stock, stock::create_stock_item))
        .routes(routes!(stock::stock_summary))
        .routes(routes!(
            stock::get_stock_item,
            stock::update_stock_item,
            stock::delete_stock_item
        ))
        .routes(routes!(stock::stock_item_history))
        .routes(routes!(stock::list_stock_photos))
}

/// Заказы, их журнал, комментарии и запчасти.
#[inline(never)]
fn order_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
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
        .routes(routes!(orders::order_parts, orders::add_order_part))
        .routes(routes!(orders::return_order_part))
        .routes(routes!(orders::accept_warranty_case))
}

/// Отчёты.
#[inline(never)]
fn report_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(reports::order_report))
        .routes(routes!(reports::order_report_xlsx))
        .routes(routes!(reports::moves_report))
}

/// Документы и шаблоны документов.
#[inline(never)]
fn document_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(documents::order_document))
        .routes(routes!(documents::sample_document))
        .routes(routes!(documents::batch_document))
        .routes(routes!(document_templates::list_templates))
        .routes(routes!(
            document_templates::get_template,
            document_templates::save_template
        ))
        .routes(routes!(document_templates::legacy_template))
        .routes(routes!(document_templates::preview_template))
        .routes(routes!(document_templates::block_code))
        .routes(routes!(
            document_templates::export_template_file,
            document_templates::import_template_file
        ))
        .routes(routes!(document_templates::list_fields))
}

/// Пользователи, роли и журнал.
#[inline(never)]
fn user_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(users::list_users, users::create_user))
        .routes(routes!(users::update_user))
        .routes(routes!(users::set_user_password))
        .routes(routes!(users::list_roles, users::create_role))
        .routes(routes!(users::update_role, users::delete_role))
        .routes(routes!(users::list_masters))
        .routes(routes!(audit::list_audit))
}

/// Импорт MyWork 2.
#[inline(never)]
fn legacy_import_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(legacy_import::check_legacy_folder))
        .routes(routes!(legacy_import::start_legacy_import))
        .routes(routes!(legacy_import::current_legacy_import))
        .routes(routes!(legacy_import::list_legacy_imports))
        .routes(routes!(legacy_import::get_legacy_import))
        .routes(routes!(
            legacy_import::legacy_stock_state,
            legacy_import::migrate_legacy_stock
        ))
}
