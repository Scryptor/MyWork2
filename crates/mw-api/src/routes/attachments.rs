//! Вложения заказов и черновиков приёма: загрузка, отдача файлов, подписи, удаление.

use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::extract::rejection::BytesRejection;
use axum::http::{HeaderName, HeaderValue, StatusCode, header};
use axum::response::Response;
use mw_core::{Attachment, AttachmentFile, AttachmentVariant, Core};
use tokio_util::io::ReaderStream;

use super::headers::{self, Disposition};
use crate::auth::CurrentActor;
use crate::dto::{AttachmentNameQuery, CaptionPatch};
use crate::error::ApiError;
use crate::extract::{Json, Path, Query};
use crate::state::AppState;

/// Предел тела запроса загрузки: файл и запас на служебные байты.
pub(super) const UPLOAD_BODY_LIMIT: usize = mw_core::attachments::MAX_ATTACHMENT_BYTES + 64 * 1024;

/// Имя файла: без пробелов по краям, 1–255 знаков.
pub(super) fn checked_name(raw: &str) -> Result<String, ApiError> {
    let name = raw.trim();
    if name.is_empty() || name.chars().count() > 255 {
        return Err(ApiError::bad_request(
            "Имя файла должно быть от 1 до 255 знаков. Укажите параметр name.",
        ));
    }
    Ok(name.to_owned())
}

/// Вложения заказа (достаточно войти).
#[utoipa::path(
    get,
    path = "/api/v1/orders/{id}/attachments",
    tag = "Вложения",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер заказа")),
    responses((status = 200, description = "Вложения: сначала фото, затем файлы", body = Vec<Attachment>))
)]
pub(super) async fn list_order_attachments(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
) -> Result<Json<Vec<Attachment>>, ApiError> {
    Ok(Json(state.core.list_order_attachments(&actor, id).await?))
}

/// Загрузка файла или фото к заказу (право `orders_edit`). Тело — сами байты,
/// `Content-Type: application/octet-stream`; не больше 25 МБ.
#[utoipa::path(
    post,
    path = "/api/v1/orders/{id}/attachments",
    tag = "Вложения",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер заказа"), AttachmentNameQuery),
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses((status = 201, description = "Вложение добавлено", body = Attachment))
)]
pub(super) async fn upload_order_attachment(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
    Query(q): Query<AttachmentNameQuery>,
    body: Result<Bytes, BytesRejection>,
) -> Result<(StatusCode, Json<Attachment>), ApiError> {
    let name = checked_name(&q.name)?;
    let bytes = body?.to_vec();
    Ok((
        StatusCode::CREATED,
        Json(
            state
                .core
                .add_order_attachment(&actor, id, &name, bytes)
                .await?,
        ),
    ))
}

/// Вложения черновика приёма (право `orders_create`).
#[utoipa::path(
    get,
    path = "/api/v1/intake-drafts/{key}/attachments",
    tag = "Вложения",
    security(("bearer" = [])),
    params(("key" = String, Path, description = "Ключ черновика")),
    responses((status = 200, description = "Вложения черновика по порядку добавления", body = Vec<Attachment>))
)]
pub(super) async fn list_draft_attachments(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(key): Path<String>,
) -> Result<Json<Vec<Attachment>>, ApiError> {
    Ok(Json(state.core.list_draft_attachments(&actor, &key).await?))
}

/// Загрузка файла или фото в черновик приёма (право `orders_create`).
#[utoipa::path(
    post,
    path = "/api/v1/intake-drafts/{key}/attachments",
    tag = "Вложения",
    security(("bearer" = [])),
    params(("key" = String, Path, description = "Ключ черновика"), AttachmentNameQuery),
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses((status = 201, description = "Вложение добавлено", body = Attachment))
)]
pub(super) async fn upload_draft_attachment(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(key): Path<String>,
    Query(q): Query<AttachmentNameQuery>,
    body: Result<Bytes, BytesRejection>,
) -> Result<(StatusCode, Json<Attachment>), ApiError> {
    let name = checked_name(&q.name)?;
    let bytes = body?.to_vec();
    Ok((
        StatusCode::CREATED,
        Json(
            state
                .core
                .add_draft_attachment(&actor, &key, &name, bytes)
                .await?,
        ),
    ))
}

/// Выбрасывает черновик приёма вместе с файлами (право `orders_create`).
#[utoipa::path(
    delete,
    path = "/api/v1/intake-drafts/{key}",
    tag = "Вложения",
    security(("bearer" = [])),
    params(("key" = String, Path, description = "Ключ черновика")),
    responses((status = 204, description = "Черновик выброшен"))
)]
pub(super) async fn discard_draft(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(key): Path<String>,
) -> Result<StatusCode, ApiError> {
    state.core.discard_draft(&actor, &key).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Подпись вложения (право `orders_edit`; у фото черновика приёма — `orders_create` или `orders_edit`;
/// у фото позиции склада — `stock_edit`).
#[utoipa::path(
    patch,
    path = "/api/v1/attachments/{id}",
    tag = "Вложения",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер вложения")),
    request_body = CaptionPatch,
    responses((status = 200, description = "Вложение после изменения", body = Attachment))
)]
pub(super) async fn update_attachment(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
    Json(patch): Json<CaptionPatch>,
) -> Result<Json<Attachment>, ApiError> {
    Ok(Json(
        state
            .core
            .update_attachment_caption(&actor, id, patch.caption)
            .await?,
    ))
}

/// Удаление вложения (право `orders_edit`; у фото черновика приёма — `orders_create` или `orders_edit`;
/// у фото позиции склада — `stock_edit`).
#[utoipa::path(
    delete,
    path = "/api/v1/attachments/{id}",
    tag = "Вложения",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер вложения")),
    responses((status = 204, description = "Вложение удалено"))
)]
pub(super) async fn delete_attachment(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    state.core.delete_attachment(&actor, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Файл вложения (достаточно войти; у фото позиции склада — право `stock_view`).
#[utoipa::path(
    get,
    path = "/api/v1/attachments/{id}/file",
    tag = "Вложения",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер вложения")),
    responses((status = 200, description = "Содержимое файла", content_type = "application/octet-stream", body = Vec<u8>))
)]
pub(super) async fn attachment_file(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
) -> Result<Response, ApiError> {
    serve(&state.core, &actor, id, AttachmentVariant::Full).await
}

/// Миниатюра фото (достаточно войти; у фото позиции склада — право `stock_view`).
#[utoipa::path(
    get,
    path = "/api/v1/attachments/{id}/thumb",
    tag = "Вложения",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер вложения")),
    responses((status = 200, description = "Миниатюра в JPEG", content_type = "image/jpeg", body = Vec<u8>))
)]
pub(super) async fn attachment_thumb(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
) -> Result<Response, ApiError> {
    serve(&state.core, &actor, id, AttachmentVariant::Thumb).await
}

async fn serve(
    core: &Core,
    actor: &mw_core::Actor,
    id: i64,
    variant: AttachmentVariant,
) -> Result<Response, ApiError> {
    let file = core.attachment_file(actor, id, variant).await?;
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
    let content_type = HeaderValue::from_str(&file.content_type)
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));
    headers.insert(header::CONTENT_TYPE, content_type);
    headers.insert(header::CONTENT_LENGTH, HeaderValue::from(len));
    if let Ok(v) = HeaderValue::from_str(&content_disposition(&file)) {
        headers.insert(header::CONTENT_DISPOSITION, v);
    }
    // Файл под данным `id` не меняется.
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=31536000, immutable"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("sandbox; default-src 'none'; frame-ancestors 'none'"),
    );
    // Чужой сайт не встроит файл к себе (в открытом режиме вход не нужен).
    headers.insert(
        HeaderName::from_static("cross-origin-resource-policy"),
        HeaderValue::from_static("same-origin"),
    );
    Ok(response)
}

/// Типы, которые браузер показывает сам: растровые фото. Остальное (в том числе SVG, который
/// может нести скрипт) — только скачиванием.
const INLINE_TYPES: [&str; 5] = [
    "image/jpeg",
    "image/png",
    "image/webp",
    "image/gif",
    "image/bmp",
];

fn content_disposition(file: &AttachmentFile) -> String {
    let kind = if INLINE_TYPES.contains(&file.content_type.as_str()) {
        Disposition::Inline
    } else {
        Disposition::Attachment
    };
    headers::content_disposition(kind, &file.name, None)
}
