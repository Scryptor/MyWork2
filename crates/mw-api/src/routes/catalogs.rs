//! Настройки, статусы, справочники и точки обслуживания.

use axum::extract::State;
use axum::http::StatusCode;
use mw_core::{
    AppSettings, DictionaryItem, NewStatus, ServicePoint, ServicePointPatch, SettingsPatch, Status,
    StatusPatch,
};
use mw_domain::dictionary::DictionaryKind;

use crate::auth::CurrentActor;
use crate::dto::{DictionaryValue, IncludeArchived, NewServicePoint, ReorderRequest};
use crate::error::ApiError;
use crate::extract::{Json, Path, Query};
use crate::state::AppState;

/// Настройки программы.
#[utoipa::path(
    get,
    path = "/api/v1/settings",
    tag = "Настройки и справочники",
    security(("bearer" = [])),
    responses((status = 200, description = "Настройки", body = AppSettings))
)]
pub(super) async fn get_settings(
    State(state): State<AppState>,
    CurrentActor(_actor): CurrentActor,
) -> Json<AppSettings> {
    Json(state.core.settings())
}

/// Изменение настроек (право `settings_manage`).
#[utoipa::path(
    patch,
    path = "/api/v1/settings",
    tag = "Настройки и справочники",
    security(("bearer" = [])),
    request_body = SettingsPatch,
    responses((status = 200, description = "Настройки после изменения", body = AppSettings))
)]
pub(super) async fn update_settings(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Json(patch): Json<SettingsPatch>,
) -> Result<Json<AppSettings>, ApiError> {
    Ok(Json(state.core.update_settings(&actor, patch).await?))
}

/// Список статусов заказов в заданном порядке.
#[utoipa::path(
    get,
    path = "/api/v1/statuses",
    tag = "Настройки и справочники",
    security(("bearer" = [])),
    params(IncludeArchived),
    responses((status = 200, description = "Статусы", body = Vec<Status>))
)]
pub(super) async fn list_statuses(
    State(state): State<AppState>,
    CurrentActor(_actor): CurrentActor,
    Query(q): Query<IncludeArchived>,
) -> Result<Json<Vec<Status>>, ApiError> {
    Ok(Json(
        state
            .core
            .list_statuses(q.include_archived.unwrap_or(false))
            .await?,
    ))
}

/// Новый статус (право `settings_manage`).
#[utoipa::path(
    post,
    path = "/api/v1/statuses",
    tag = "Настройки и справочники",
    security(("bearer" = [])),
    request_body = NewStatus,
    responses((status = 201, description = "Статус создан", body = Status))
)]
pub(super) async fn create_status(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Json(new): Json<NewStatus>,
) -> Result<(StatusCode, Json<Status>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(state.core.create_status(&actor, new).await?),
    ))
}

/// Изменение статуса: название, цвет, архивация (право `settings_manage`).
#[utoipa::path(
    patch,
    path = "/api/v1/statuses/{id}",
    tag = "Настройки и справочники",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер статуса")),
    request_body = StatusPatch,
    responses((status = 200, description = "Статус после изменения", body = Status))
)]
pub(super) async fn update_status(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
    Json(patch): Json<StatusPatch>,
) -> Result<Json<Status>, ApiError> {
    Ok(Json(state.core.update_status(&actor, id, patch).await?))
}

/// Новый порядок статусов: нужны номера всех статусов, включая архивные.
#[utoipa::path(
    put,
    path = "/api/v1/statuses/order",
    tag = "Настройки и справочники",
    security(("bearer" = [])),
    request_body = ReorderRequest,
    responses((status = 204, description = "Порядок сохранён"))
)]
pub(super) async fn reorder_statuses(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Json(req): Json<ReorderRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .core
        .reorder_statuses(&actor, &req.ordered_ids)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Значения справочника в заданном порядке. Неизвестный вид — 400.
#[utoipa::path(
    get,
    path = "/api/v1/dictionaries/{kind}",
    tag = "Настройки и справочники",
    security(("bearer" = [])),
    params(("kind" = DictionaryKind, Path, description = "Вид справочника")),
    responses((status = 200, description = "Значения", body = Vec<DictionaryItem>))
)]
pub(super) async fn list_dictionary(
    State(state): State<AppState>,
    CurrentActor(_actor): CurrentActor,
    Path(kind): Path<DictionaryKind>,
) -> Result<Json<Vec<DictionaryItem>>, ApiError> {
    Ok(Json(state.core.list_dictionary(kind).await?))
}

/// Новое значение справочника; доступно и при приёме заказа.
#[utoipa::path(
    post,
    path = "/api/v1/dictionaries/{kind}",
    tag = "Настройки и справочники",
    security(("bearer" = [])),
    params(("kind" = DictionaryKind, Path, description = "Вид справочника")),
    request_body = DictionaryValue,
    responses((status = 201, description = "Значение добавлено", body = DictionaryItem))
)]
pub(super) async fn add_dictionary_item(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(kind): Path<DictionaryKind>,
    Json(req): Json<DictionaryValue>,
) -> Result<(StatusCode, Json<DictionaryItem>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(
            state
                .core
                .add_dictionary_item(&actor, kind, &req.value)
                .await?,
        ),
    ))
}

/// Новый порядок значений справочника: нужны номера всех его значений.
#[utoipa::path(
    put,
    path = "/api/v1/dictionaries/{kind}/order",
    tag = "Настройки и справочники",
    security(("bearer" = [])),
    params(("kind" = DictionaryKind, Path, description = "Вид справочника")),
    request_body = ReorderRequest,
    responses((status = 204, description = "Порядок сохранён"))
)]
pub(super) async fn reorder_dictionary(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(kind): Path<DictionaryKind>,
    Json(req): Json<ReorderRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .core
        .reorder_dictionary(&actor, kind, &req.ordered_ids)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Переименование значения справочника (право `settings_manage`).
#[utoipa::path(
    patch,
    path = "/api/v1/dictionary-items/{id}",
    tag = "Настройки и справочники",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер значения")),
    request_body = DictionaryValue,
    responses((status = 200, description = "Значение после изменения", body = DictionaryItem))
)]
pub(super) async fn rename_dictionary_item(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
    Json(req): Json<DictionaryValue>,
) -> Result<Json<DictionaryItem>, ApiError> {
    Ok(Json(
        state
            .core
            .rename_dictionary_item(&actor, id, &req.value)
            .await?,
    ))
}

/// Удаление значения справочника (право `settings_manage`).
#[utoipa::path(
    delete,
    path = "/api/v1/dictionary-items/{id}",
    tag = "Настройки и справочники",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер значения")),
    responses((status = 204, description = "Значение удалено"))
)]
pub(super) async fn delete_dictionary_item(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    state.core.delete_dictionary_item(&actor, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Точки обслуживания в заданном порядке.
#[utoipa::path(
    get,
    path = "/api/v1/service-points",
    tag = "Настройки и справочники",
    security(("bearer" = [])),
    params(IncludeArchived),
    responses((status = 200, description = "Точки обслуживания", body = Vec<ServicePoint>))
)]
pub(super) async fn list_service_points(
    State(state): State<AppState>,
    CurrentActor(_actor): CurrentActor,
    Query(q): Query<IncludeArchived>,
) -> Result<Json<Vec<ServicePoint>>, ApiError> {
    Ok(Json(
        state
            .core
            .list_service_points(q.include_archived.unwrap_or(false))
            .await?,
    ))
}

/// Новая точка обслуживания (право `settings_manage`).
#[utoipa::path(
    post,
    path = "/api/v1/service-points",
    tag = "Настройки и справочники",
    security(("bearer" = [])),
    request_body = NewServicePoint,
    responses((status = 201, description = "Точка создана", body = ServicePoint))
)]
pub(super) async fn create_service_point(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Json(req): Json<NewServicePoint>,
) -> Result<(StatusCode, Json<ServicePoint>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(state.core.create_service_point(&actor, &req.name).await?),
    ))
}

/// Изменение точки обслуживания: название, архивация (право `settings_manage`).
#[utoipa::path(
    patch,
    path = "/api/v1/service-points/{id}",
    tag = "Настройки и справочники",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер точки")),
    request_body = ServicePointPatch,
    responses((status = 200, description = "Точка после изменения", body = ServicePoint))
)]
pub(super) async fn update_service_point(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
    Json(patch): Json<ServicePointPatch>,
) -> Result<Json<ServicePoint>, ApiError> {
    Ok(Json(
        state.core.update_service_point(&actor, id, patch).await?,
    ))
}

/// Новый порядок точек: нужны номера всех точек, включая архивные.
#[utoipa::path(
    put,
    path = "/api/v1/service-points/order",
    tag = "Настройки и справочники",
    security(("bearer" = [])),
    request_body = ReorderRequest,
    responses((status = 204, description = "Порядок сохранён"))
)]
pub(super) async fn reorder_service_points(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Json(req): Json<ReorderRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .core
        .reorder_service_points(&actor, &req.ordered_ids)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
