//! Логотип фирмы: загрузка, отдача, удаление.

use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::extract::rejection::BytesRejection;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;
use mw_core::FirmLogo;
use tokio_util::io::ReaderStream;

use crate::auth::CurrentActor;
use crate::error::ApiError;
use crate::extract::Json;
use crate::state::AppState;

/// Предел тела загрузки: файл до 5 МиБ проверяет ядро (422), с запасом на служебные байты.
pub(super) const LOGO_BODY_LIMIT: usize = 6 * 1024 * 1024;

fn no_logo() -> ApiError {
    ApiError::from(mw_core::CoreError::NotFound {
        entity: "логотип",
        id: 0,
    })
}

/// Картинка логотипа (достаточно войти); 404, если логотипа нет.
#[utoipa::path(
    get,
    path = "/api/v1/settings/logo",
    tag = "Настройки и справочники",
    security(("bearer" = [])),
    responses((status = 200, description = "Логотип в PNG", content_type = "image/png", body = Vec<u8>))
)]
pub(super) async fn get_logo(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
) -> Result<Response, ApiError> {
    let file = state
        .core
        .firm_logo_file(&actor)
        .await?
        .ok_or_else(no_logo)?;
    let opened = tokio::fs::File::open(&file.path)
        .await
        .map_err(|_| ApiError::file_missing())?;
    let len = opened
        .metadata()
        .await
        .map_err(|_| ApiError::file_missing())?
        .len();
    let mut response = Response::new(Body::from_stream(ReaderStream::new(opened)));
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/png"));
    headers.insert(header::CONTENT_LENGTH, HeaderValue::from(len));
    // Логотип меняется под тем же адресом.
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    Ok(response)
}

/// Сведения о логотипе (достаточно войти); 404, если логотипа нет.
#[utoipa::path(
    get,
    path = "/api/v1/settings/logo/info",
    tag = "Настройки и справочники",
    security(("bearer" = [])),
    responses((status = 200, description = "Размеры и размер файла", body = FirmLogo))
)]
pub(super) async fn get_logo_info(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
) -> Result<Json<FirmLogo>, ApiError> {
    Ok(Json(
        state.core.firm_logo(&actor).await?.ok_or_else(no_logo)?,
    ))
}

/// Загрузка логотипа (право `settings_manage`). Тело — байты PNG, JPEG или WebP,
/// `Content-Type: application/octet-stream`, не больше 5 МиБ; прежний логотип заменяется.
#[utoipa::path(
    put,
    path = "/api/v1/settings/logo",
    tag = "Настройки и справочники",
    security(("bearer" = [])),
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses((status = 200, description = "Логотип после обработки", body = FirmLogo))
)]
pub(super) async fn put_logo(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    body: Result<Bytes, BytesRejection>,
) -> Result<Json<FirmLogo>, ApiError> {
    let bytes = body?.to_vec();
    Ok(Json(state.core.set_firm_logo(&actor, bytes).await?))
}

/// Удаление логотипа (право `settings_manage`); без логотипа ничего не меняет.
#[utoipa::path(
    delete,
    path = "/api/v1/settings/logo",
    tag = "Настройки и справочники",
    security(("bearer" = [])),
    responses((status = 204, description = "Логотипа нет"))
)]
pub(super) async fn delete_logo(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
) -> Result<StatusCode, ApiError> {
    state.core.remove_firm_logo(&actor).await?;
    Ok(StatusCode::NO_CONTENT)
}
