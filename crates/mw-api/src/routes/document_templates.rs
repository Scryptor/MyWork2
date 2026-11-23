//! Шаблоны печатных документов: правка, превью черновика, файл `.mwtemplate`.

use axum::extract::State;
use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};
use mw_core::documents::{DocumentBytes, DocumentOutput, DocumentRequest};
use mw_core::{
    DocumentField, DocumentTemplate, DocumentTemplateInfo, LegacyTemplate, TemplateSave,
};
use mw_domain::document_template::{Template, TemplateFile};
use mw_domain::documents::DocumentKind;

use super::documents::{DEFAULT_PNG_DPI, file_base, png_response, render_options};
use crate::auth::CurrentActor;
use crate::dto::{DocumentPages, TemplatePreview};
use crate::error::ApiError;
use crate::extract::{Json, Path};
use crate::state::AppState;

/// Виды документов, шаблоны которых можно менять (достаточно войти).
#[utoipa::path(
    get,
    path = "/api/v1/document-templates",
    tag = "Документы",
    security(("bearer" = [])),
    responses((status = 200, body = Vec<DocumentTemplateInfo>))
)]
pub(super) async fn list_templates(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
) -> Result<Json<Vec<DocumentTemplateInfo>>, ApiError> {
    Ok(Json(state.core.list_document_templates(&actor).await?))
}

/// Шаблон вида со стандартным образцом (достаточно войти). Калибровка не редактируется: 422.
#[utoipa::path(
    get,
    path = "/api/v1/document-templates/{kind}",
    tag = "Документы",
    security(("bearer" = [])),
    params(("kind" = DocumentKind, Path, description = "Вид документа")),
    responses((status = 200, body = DocumentTemplate))
)]
pub(super) async fn get_template(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(kind): Path<DocumentKind>,
) -> Result<Json<DocumentTemplate>, ApiError> {
    Ok(Json(state.core.get_document_template(&actor, kind).await?))
}

/// Текст старого шаблона акта MyWork 2 с метками, заменёнными полями (достаточно войти). 404 —
/// у вида нет старого файла или импорта не было.
#[utoipa::path(
    get,
    path = "/api/v1/document-templates/{kind}/legacy",
    tag = "Документы",
    security(("bearer" = [])),
    params(("kind" = DocumentKind, Path, description = "Вид документа")),
    responses((status = 200, body = LegacyTemplate))
)]
pub(super) async fn legacy_template(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(kind): Path<DocumentKind>,
) -> Result<Json<LegacyTemplate>, ApiError> {
    state
        .core
        .legacy_document_template(&actor, kind)
        .await?
        .map(Json)
        .ok_or_else(ApiError::no_legacy_template)
}

/// Сохраняет шаблон по номеру правки (право «настройки»). Модель, равная стандартной, сбрасывает
/// шаблон. Устаревшая `version` — 409, неверная модель — 422 по полям.
#[utoipa::path(
    put,
    path = "/api/v1/document-templates/{kind}",
    tag = "Документы",
    security(("bearer" = [])),
    params(("kind" = DocumentKind, Path, description = "Вид документа")),
    request_body = TemplateSave,
    responses((status = 200, body = DocumentTemplate))
)]
pub(super) async fn save_template(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(kind): Path<DocumentKind>,
    Json(save): Json<TemplateSave>,
) -> Result<Json<DocumentTemplate>, ApiError> {
    Ok(Json(
        state
            .core
            .save_document_template(&actor, kind, save)
            .await?,
    ))
}

/// Превью черновика шаблона в PNG, без сохранения (право «настройки»). Без `order_id` — образец.
#[utoipa::path(
    post,
    path = "/api/v1/document-templates/{kind}/preview",
    tag = "Документы",
    security(("bearer" = [])),
    params(("kind" = DocumentKind, Path, description = "Вид документа")),
    request_body = TemplatePreview,
    responses((status = 200, body = DocumentPages))
)]
pub(super) async fn preview_template(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(kind): Path<DocumentKind>,
    Json(body): Json<TemplatePreview>,
) -> Result<Response, ApiError> {
    let options = render_options(
        (body.label_w_mm, body.label_h_mm),
        (body.offset_x_mm, body.offset_y_mm),
        body.printer_dpi,
    )?;
    let dpi = body.dpi.unwrap_or(DEFAULT_PNG_DPI);
    let request = DocumentRequest {
        kind,
        order_id: body.order_id,
        sample: body.order_id.is_none() && kind.for_order(),
        options,
        draft: Some(body.template),
    };
    let file = state
        .core
        .export_document(&actor, request, DocumentOutput::Png { dpi })
        .await?;
    let DocumentBytes::Png(pages) = file.bytes else {
        return Err(mw_core::CoreError::Internal("Превью вернуло не PNG".into()).into());
    };
    let mut response = png_response(&pages, file.page_size_mm, dpi);
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    Ok(response)
}

/// Шаблон как файл `.mwtemplate` (достаточно войти).
#[utoipa::path(
    get,
    path = "/api/v1/document-templates/{kind}/file",
    tag = "Документы",
    security(("bearer" = [])),
    params(("kind" = DocumentKind, Path, description = "Вид документа")),
    responses((status = 200, description = "Файл шаблона (вложение)", content(
        (TemplateFile = "application/json")
    )))
)]
pub(super) async fn export_template_file(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(kind): Path<DocumentKind>,
) -> Result<Response, ApiError> {
    let template = state.core.get_document_template(&actor, kind).await?;
    let mut response = Json(TemplateFile::new(template.template)).into_response();
    let name = format!("{}.mwtemplate", file_base(kind));
    if let Ok(v) = HeaderValue::from_str(&format!("attachment; filename=\"{name}\"")) {
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, v);
    }
    Ok(response)
}

/// Разбирает файл `.mwtemplate` для вида и возвращает модель; ничего не сохраняет (право
/// «настройки»). Ошибки формата — 422 по полям.
#[utoipa::path(
    post,
    path = "/api/v1/document-templates/{kind}/file",
    tag = "Документы",
    security(("bearer" = [])),
    params(("kind" = DocumentKind, Path, description = "Вид документа")),
    request_body = TemplateFile,
    responses((status = 200, body = Template))
)]
pub(super) async fn import_template_file(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(kind): Path<DocumentKind>,
    Json(file): Json<serde_json::Value>,
) -> Result<Json<Template>, ApiError> {
    Ok(Json(
        state
            .core
            .import_document_template(&actor, kind, file)
            .await?,
    ))
}

/// Поля подстановки для меню «вставить поле» (достаточно войти).
#[utoipa::path(
    get,
    path = "/api/v1/document-fields",
    tag = "Документы",
    security(("bearer" = [])),
    responses((status = 200, body = Vec<DocumentField>))
)]
pub(super) async fn list_fields(
    State(state): State<AppState>,
    CurrentActor(_actor): CurrentActor,
) -> Json<Vec<DocumentField>> {
    Json(state.core.document_fields())
}
