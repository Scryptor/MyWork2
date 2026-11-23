//! Документы заказа в PDF и PNG.

use axum::extract::State;
use axum::http::header;
use axum::response::{IntoResponse, Response};
use base64::Engine as _;
use mw_core::CoreError;
use mw_core::documents::{DocumentBatchRequest, DocumentBytes, DocumentOutput, DocumentRequest};
use mw_docs::RenderOptions;
use mw_domain::documents::DocumentKind;

use crate::auth::CurrentActor;
use crate::dto::{DocumentBatch, DocumentFormat, DocumentPages, DocumentQuery};
use crate::error::ApiError;
use crate::extract::{Json, Path, Query};
use crate::state::AppState;

use super::headers::{Disposition, no_store, with_disposition};

/// Разрешение растра PNG по умолчанию: хватает для предпросмотра на экране.
pub(super) const DEFAULT_PNG_DPI: u32 = 110;

/// Документ заказа: акт приёма или выдачи, гарантийный акт, наклейка (достаточно войти; права те же,
/// что у просмотра заказа). Пустой акт приёма и калибровка заказа не используют.
#[utoipa::path(
    get,
    path = "/api/v1/orders/{id}/documents/{kind}",
    tag = "Документы",
    security(("bearer" = [])),
    params(
        ("id" = i64, Path, description = "Номер заказа"),
        ("kind" = DocumentKind, Path, description = "Вид документа"),
        DocumentQuery
    ),
    responses(
        (status = 200, description = "PDF или, для `format=png`, страницы PNG", content(
            (Vec<u8> = "application/pdf"),
            (DocumentPages = "application/json")
        ))
    )
)]
pub(super) async fn order_document(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path((id, kind)): Path<(i64, DocumentKind)>,
    Query(q): Query<DocumentQuery>,
) -> Result<Response, ApiError> {
    respond(&state, &actor, kind, Some(id), q).await
}

/// Документ без заказа: пустой акт приёма и калибровочная этикетка как есть, остальные виды —
/// образец на примерных данных и своих реквизитах (достаточно войти).
#[utoipa::path(
    get,
    path = "/api/v1/documents/{kind}",
    tag = "Документы",
    security(("bearer" = [])),
    params(
        ("kind" = DocumentKind, Path, description = "Вид документа"),
        DocumentQuery
    ),
    responses(
        (status = 200, description = "PDF или, для `format=png`, страницы PNG", content(
            (Vec<u8> = "application/pdf"),
            (DocumentPages = "application/json")
        ))
    )
)]
pub(super) async fn sample_document(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(kind): Path<DocumentKind>,
    Query(q): Query<DocumentQuery>,
) -> Result<Response, ApiError> {
    respond(&state, &actor, kind, None, q).await
}

/// Документы нескольких заказов одним PDF (вид из заказа; не больше 100 заказов, 50 страниц на
/// заказ и 200 всего). Права те же, что у документа одного заказа.
#[utoipa::path(
    post,
    path = "/api/v1/documents/{kind}/batch",
    tag = "Документы",
    security(("bearer" = [])),
    params(("kind" = DocumentKind, Path, description = "Вид документа")),
    request_body = DocumentBatch,
    responses((status = 200, description = "PDF", content((Vec<u8> = "application/pdf"))))
)]
pub(super) async fn batch_document(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(kind): Path<DocumentKind>,
    Json(body): Json<DocumentBatch>,
) -> Result<Response, ApiError> {
    let options = render_options(
        (body.label_w_mm, body.label_h_mm),
        (body.offset_x_mm, body.offset_y_mm),
        body.printer_dpi,
    )?;
    // Число заказов в имени файла — после удаления повторов.
    let count = body
        .order_ids
        .iter()
        .collect::<std::collections::HashSet<_>>()
        .len();
    let request = DocumentBatchRequest {
        kind,
        order_ids: body.order_ids,
        options,
    };
    let file = state
        .core
        .export_documents(&actor, request)
        .await
        .map_err(|e| match e {
            CoreError::NotFound {
                entity: "заказ",
                id,
            } => ApiError::order_not_found(id),
            e => e.into(),
        })?;
    let DocumentBytes::Pdf(pdf) = file.bytes else {
        return Err(CoreError::Internal("пачка документов вернула не PDF".into()).into());
    };
    Ok(no_store(pdf_response(
        pdf,
        &format!("{}-{count}.pdf", file_base(kind)),
    )))
}

async fn respond(
    state: &AppState,
    actor: &mw_core::Actor,
    kind: DocumentKind,
    order_id: Option<i64>,
    q: DocumentQuery,
) -> Result<Response, ApiError> {
    let format = q.format.unwrap_or_default();
    if format == DocumentFormat::Pdf && q.dpi.is_some() {
        return Err(ApiError::bad_request(
            "Параметр dpi задаёт разрешение растра и нужен только для формата png",
        ));
    }
    let options = render_options(
        (q.label_w_mm, q.label_h_mm),
        (q.offset_x_mm, q.offset_y_mm),
        q.printer_dpi,
    )?;
    let raster_dpi = q.dpi.unwrap_or(DEFAULT_PNG_DPI);
    let output = match format {
        DocumentFormat::Pdf => DocumentOutput::Pdf,
        DocumentFormat::Png => DocumentOutput::Png { dpi: raster_dpi },
    };
    let request = DocumentRequest {
        kind,
        order_id,
        sample: order_id.is_none() && kind.for_order(),
        options,
        draft: None,
    };
    let file = state.core.export_document(actor, request, output).await?;
    let response = match file.bytes {
        DocumentBytes::Pdf(pdf) => {
            let name = file_name(kind, order_id.filter(|_| kind.for_order()));
            pdf_response(pdf, &name)
        }
        DocumentBytes::Png(pages) => png_response(&pages, file.page_size_mm, raster_dpi),
    };
    Ok(no_store(response))
}

/// PDF с именем файла для показа в окне.
fn pdf_response(pdf: Vec<u8>, name: &str) -> Response {
    with_disposition(
        ([(header::CONTENT_TYPE, "application/pdf")], pdf).into_response(),
        Disposition::Inline,
        name,
    )
}

/// Параметры отрисовки из запроса: размер наклейки — только парой.
pub(super) fn render_options(
    label_mm: (Option<f32>, Option<f32>),
    offset_mm: (Option<f32>, Option<f32>),
    printer_dpi: Option<u32>,
) -> Result<RenderOptions, ApiError> {
    let label_mm = match label_mm {
        (Some(w), Some(h)) => Some((w, h)),
        (None, None) => None,
        _ => {
            return Err(ApiError::bad_request(
                "Размер наклейки задаётся парой: укажите label_w_mm и label_h_mm вместе",
            ));
        }
    };
    Ok(RenderOptions {
        label_mm,
        offset_mm: (offset_mm.0.unwrap_or(0.0), offset_mm.1.unwrap_or(0.0)),
        dpi: printer_dpi,
    })
}

/// Страницы PNG как `DocumentPages` (base64).
pub(super) fn png_response(pages: &[Vec<u8>], page_size_mm: (f64, f64), dpi: u32) -> Response {
    let engine = base64::engine::general_purpose::STANDARD;
    Json(DocumentPages {
        dpi,
        width_mm: page_size_mm.0 as f32,
        height_mm: page_size_mm.1 as f32,
        pages: pages.iter().map(|png| engine.encode(png)).collect(),
    })
    .into_response()
}

/// Основа имени файла: латиница и цифры, чтобы заголовок остался ASCII.
pub(super) fn file_base(kind: DocumentKind) -> &'static str {
    match kind {
        DocumentKind::IntakeAct => "akt-priema",
        DocumentKind::IssueAct => "akt-vydachi",
        DocumentKind::WarrantyIntakeAct => "akt-priema-garantiya",
        DocumentKind::WarrantyIssueAct => "akt-vydachi-garantiya",
        DocumentKind::BlankIntakeAct => "akt-priema-pustoj",
        DocumentKind::Sticker => "naklejka",
        DocumentKind::Calibration => "kalibrovka",
    }
}

/// Имя файла PDF.
fn file_name(kind: DocumentKind, order_number: Option<i64>) -> String {
    let base = file_base(kind);
    match order_number {
        Some(number) => format!("{base}-{number}.pdf"),
        None => format!("{base}.pdf"),
    }
}
