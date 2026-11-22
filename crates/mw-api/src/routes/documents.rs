//! Документы заказа в PDF и PNG.

use axum::extract::State;
use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};
use base64::Engine as _;
use mw_core::documents::{DocumentBytes, DocumentOutput, DocumentRequest};
use mw_docs::RenderOptions;
use mw_domain::documents::DocumentKind;

use crate::auth::CurrentActor;
use crate::dto::{DocumentFormat, DocumentPages, DocumentQuery};
use crate::error::ApiError;
use crate::extract::{Json, Path, Query};
use crate::state::AppState;

/// Разрешение растра PNG по умолчанию: хватает для предпросмотра на экране.
const DEFAULT_PNG_DPI: u32 = 110;

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
    let label_mm = match (q.label_w_mm, q.label_h_mm) {
        (Some(w), Some(h)) => Some((w, h)),
        (None, None) => None,
        _ => {
            return Err(ApiError::bad_request(
                "Размер наклейки задаётся парой: укажите label_w_mm и label_h_mm вместе",
            ));
        }
    };
    let raster_dpi = q.dpi.unwrap_or(DEFAULT_PNG_DPI);
    let output = match format {
        DocumentFormat::Pdf => DocumentOutput::Pdf,
        DocumentFormat::Png => DocumentOutput::Png { dpi: raster_dpi },
    };
    let request = DocumentRequest {
        kind,
        order_id,
        sample: order_id.is_none() && kind.for_order(),
        options: RenderOptions {
            label_mm,
            offset_mm: (q.offset_x_mm.unwrap_or(0.0), q.offset_y_mm.unwrap_or(0.0)),
            dpi: q.printer_dpi,
        },
    };
    let file = state.core.export_document(actor, request, output).await?;
    let mut response = match file.bytes {
        DocumentBytes::Pdf(pdf) => {
            let name = file_name(kind, order_id.filter(|_| kind.for_order()));
            let mut response = ([(header::CONTENT_TYPE, "application/pdf")], pdf).into_response();
            if let Ok(v) = HeaderValue::from_str(&format!("inline; filename=\"{name}\"")) {
                response
                    .headers_mut()
                    .insert(header::CONTENT_DISPOSITION, v);
            }
            response
        }
        DocumentBytes::Png(pages) => {
            let engine = base64::engine::general_purpose::STANDARD;
            Json(DocumentPages {
                dpi: raster_dpi,
                width_mm: file.page_size_mm.0 as f32,
                height_mm: file.page_size_mm.1 as f32,
                pages: pages.iter().map(|png| engine.encode(png)).collect(),
            })
            .into_response()
        }
    };
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    Ok(response)
}

/// Имя файла: латиница и цифры, чтобы заголовок остался ASCII.
fn file_name(kind: DocumentKind, order_number: Option<i64>) -> String {
    let base = match kind {
        DocumentKind::IntakeAct => "akt-priema",
        DocumentKind::IssueAct => "akt-vydachi",
        DocumentKind::WarrantyIntakeAct => "akt-priema-garantiya",
        DocumentKind::WarrantyIssueAct => "akt-vydachi-garantiya",
        DocumentKind::BlankIntakeAct => "akt-priema-pustoj",
        DocumentKind::Sticker => "naklejka",
        DocumentKind::Calibration => "kalibrovka",
    };
    match order_number {
        Some(number) => format!("{base}-{number}.pdf"),
        None => format!("{base}.pdf"),
    }
}
