//! Клиенты.

use axum::extract::State;
use axum::http::StatusCode;
use mw_core::{Client, ClientPatch, NewClient};

use crate::DEFAULT_PAGE_LIMIT;
use crate::auth::CurrentActor;
use crate::dto::{ClientListQuery, ClientPage, PhoneQuery, VersionParam};
use crate::error::ApiError;
use crate::extract::{Json, Path, Query};
use crate::state::AppState;

/// Список и поиск клиентов (право `clients_view`).
#[utoipa::path(
    get,
    path = "/api/v1/clients",
    tag = "Клиенты",
    security(("bearer" = [])),
    params(ClientListQuery),
    responses((status = 200, description = "Страница клиентов", body = ClientPage))
)]
pub(super) async fn list_clients(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Query(q): Query<ClientListQuery>,
) -> Result<Json<ClientPage>, ApiError> {
    let page = state
        .core
        .find_clients(
            &actor,
            q.query.as_deref().unwrap_or(""),
            q.limit.unwrap_or(DEFAULT_PAGE_LIMIT),
            q.offset.unwrap_or(0),
        )
        .await?;
    Ok(Json(page.into()))
}

/// Клиенты с указанным телефоном (право `clients_view`).
#[utoipa::path(
    get,
    path = "/api/v1/clients/by-phone",
    tag = "Клиенты",
    security(("bearer" = [])),
    params(PhoneQuery),
    responses((status = 200, description = "Найденные клиенты", body = Vec<Client>))
)]
pub(super) async fn clients_by_phone(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Query(q): Query<PhoneQuery>,
) -> Result<Json<Vec<Client>>, ApiError> {
    Ok(Json(
        state.core.find_clients_by_phone(&actor, &q.phone).await?,
    ))
}

/// Карточка клиента (право `clients_view`).
#[utoipa::path(
    get,
    path = "/api/v1/clients/{id}",
    tag = "Клиенты",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер клиента")),
    responses((status = 200, description = "Клиент", body = Client))
)]
pub(super) async fn get_client(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
) -> Result<Json<Client>, ApiError> {
    Ok(Json(state.core.get_client(&actor, id).await?))
}

/// Новый клиент (право `clients_create`).
#[utoipa::path(
    post,
    path = "/api/v1/clients",
    tag = "Клиенты",
    security(("bearer" = [])),
    request_body = NewClient,
    responses((status = 201, description = "Клиент создан", body = Client))
)]
pub(super) async fn create_client(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Json(new): Json<NewClient>,
) -> Result<(StatusCode, Json<Client>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(state.core.create_client(&actor, new).await?),
    ))
}

/// Изменение клиента по версии записи (право `clients_edit`). Правка без различий
/// успешна и версию не проверяет.
#[utoipa::path(
    patch,
    path = "/api/v1/clients/{id}",
    tag = "Клиенты",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер клиента"), VersionParam),
    request_body = ClientPatch,
    responses((status = 200, description = "Клиент после изменения", body = Client))
)]
pub(super) async fn update_client(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
    Query(v): Query<VersionParam>,
    Json(patch): Json<ClientPatch>,
) -> Result<Json<Client>, ApiError> {
    Ok(Json(
        state
            .core
            .update_client(&actor, id, v.version, patch)
            .await?,
    ))
}
