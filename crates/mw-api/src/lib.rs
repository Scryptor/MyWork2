//! HTTP API MyWork2: Axum-роутер поверх `mw-core`.

mod auth;
mod contract;
mod dto;
mod error;
mod extract;
mod limiter;
mod middleware;
mod routes;
mod state;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::extract::DefaultBodyLimit;
use axum::middleware::from_fn;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::openapi::{
    ContentBuilder, InfoBuilder, LicenseBuilder, OpenApi, RefOr, ResponseBuilder, Schema,
};
use utoipa::{Modify, OpenApi as _, ToSchema};

pub use auth::{ACCOUNT_FAILURE_LIMIT, FAILURE_WINDOW_SECS, IP_FAILURE_LIMIT, OPEN_MODE_ACTOR};
pub use contract::export_typescript;
pub use dto::{
    AuditListQuery, AuditPage, BarcodeQuery, ChangePasswordRequest, ClientListQuery, ClientPage,
    CurrentUser, DictionaryValue, IncludeArchived, LoginRequest, LoginResponse, NewServicePoint,
    NextOrderNumber, OrderListQuery, OrderPage, OrderSummaryQuery, OrderValuesQuery, PhoneQuery,
    ReorderRequest, ServerInfo, SetPasswordRequest, StatusChange, VersionParam,
};
pub use error::{ErrorBody, ErrorCode, ErrorField};
pub use state::ApiConfig;

use middleware::{
    method_not_allowed_fallback, not_found_fallback, request_log, require_json_content_type,
    require_local_host, security_headers, with_timeout,
};
use state::{AppState, Limits};

/// Версия API (путь `/api/v1`).
pub const API_VERSION: u32 = 1;
/// Размер страницы списков по умолчанию.
pub const DEFAULT_PAGE_LIMIT: u64 = 50;
/// Наибольшее время обработки запроса.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Наибольший размер тела запроса.
const BODY_LIMIT: usize = 1 << 20;

const API_DESCRIPTION: &str = "\
HTTP API учёта сервисного центра MyWork 3.

**Вход.** `POST /api/v1/auth/login` выдаёт токен; дальше он передаётся в заголовке \
`Authorization: Bearer <токен>`. Если сервер слушает только локальный адрес и в базе нет \
активных пользователей, вход не нужен (режим без входа): запросы без токена выполняются \
с полными правами. Узнать это можно по `GET /api/v1/server-info` (`auth_required`).

**Форматы.** Поля JSON — `snake_case`. Деньги — целое число копеек. Даты и время — местное \
время предприятия без смещения, `YYYY-MM-DDTHH:MM:SS` (в схемах помечены `date-time`, но \
смещения в них нет).

**Правки.** В теле `PATCH` непереданное поле не меняется; `null` очищает только те поля, \
которые бывают пустыми; `null` у остальных полей и неизвестные поля — ошибка 400.

**Версия записи.** Изменение заказов, клиентов, пользователей и ролей принимает параметр \
`version` — версию записи, которую видел клиент; устаревшая даёт 409 с `current_version`.

**Тело запроса.** Каждый изменяющий запрос (POST, PUT, PATCH, DELETE) должен нести \
`Content-Type: application/json`, даже без тела (выход, выдача, удаление), иначе ответ 415.

**Ошибки.** Любая ошибка — JSON `ErrorBody` с полями `code`, `message` (по-русски), `fields`, \
`current_version`, `permission`, `retry_after_secs`. Коды: `bad_request` (400, 405, 413 — тело больше 1 МиБ, 415), \
`unauthenticated` и `invalid_credentials` (401), `forbidden` (403), `not_found` (404), \
`conflict` (409), `validation` (422), `too_many_attempts` (429), `unavailable` (503), \
`internal` (500).";

/// Общая часть документа: сведения, схема безопасности, ответ `default` и схемы ошибок.
struct CommonDoc;

impl Modify for CommonDoc {
    fn modify(&self, openapi: &mut OpenApi) {
        openapi.info = InfoBuilder::new()
            .title("MyWork 3 API")
            .version(env!("CARGO_PKG_VERSION"))
            .description(Some(API_DESCRIPTION))
            .license(Some(
                LicenseBuilder::new()
                    .name("MIT")
                    .identifier(Some("MIT"))
                    .build(),
            ))
            .build();

        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "bearer",
            SecurityScheme::Http(HttpBuilder::new().scheme(HttpAuthScheme::Bearer).build()),
        );
        let mut schemas: Vec<(String, RefOr<Schema>)> = Vec::new();
        ErrorBody::schemas(&mut schemas);
        schemas.push((
            ErrorBody::name().into_owned(),
            <ErrorBody as utoipa::PartialSchema>::schema(),
        ));
        for (name, schema) in schemas {
            components.schemas.entry(name).or_insert(schema);
        }

        let error_response = || {
            ResponseBuilder::new()
                .description("Ошибка")
                .content(
                    "application/json",
                    ContentBuilder::new()
                        .schema(Some(utoipa::openapi::Ref::from_schema_name("ErrorBody")))
                        .build(),
                )
                .build()
        };
        for item in openapi.paths.paths.values_mut() {
            let operations = [
                &mut item.get,
                &mut item.put,
                &mut item.post,
                &mut item.delete,
                &mut item.patch,
            ];
            for operation in operations.into_iter().flatten() {
                operation
                    .responses
                    .responses
                    .entry("default".to_owned())
                    .or_insert_with(|| error_response().into());
            }
        }
    }
}

#[derive(utoipa::OpenApi)]
#[openapi(components(schemas(
    mw_domain::dictionary::DictionaryKind,
    mw_core::IssuedFilter,
    mw_core::OrderSortField,
    mw_core::OrderValueField
)))]
struct ApiDoc;

/// Маршруты и документ; единый источник для `router` и `openapi`.
fn build() -> (Router<AppState>, OpenApi) {
    let (router, mut api) = utoipa_axum::router::OpenApiRouter::with_openapi(ApiDoc::openapi())
        .merge(routes::api_routes())
        .split_for_parts();
    CommonDoc.modify(&mut api);
    (router, api)
}

/// Документ OpenAPI всего API.
pub fn openapi() -> OpenApi {
    build().1
}

/// Готовый роутер со всеми слоями защиты.
pub fn router(core: mw_core::Core, config: ApiConfig) -> Router {
    let (routes, api) = build();
    let openapi_json = Bytes::from(serde_json::to_vec(&api).unwrap_or_else(|e| {
        tracing::error!(error = %e, "не удалось сериализовать документ OpenAPI");
        b"{}".to_vec()
    }));
    // Слои перечислены изнутри наружу: журнал запроса — самый внешний.
    let mut app = routes
        .fallback(not_found_fallback)
        .method_not_allowed_fallback(method_not_allowed_fallback)
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
        .layer(from_fn(security_headers))
        .layer(from_fn(require_json_content_type));
    if config.loopback {
        app = app.layer(from_fn(require_local_host));
    }
    with_timeout(app, REQUEST_TIMEOUT)
        .layer(from_fn(request_log))
        .with_state(AppState {
            core,
            config,
            openapi_json,
            limits: Arc::new(Limits::new()),
        })
}
