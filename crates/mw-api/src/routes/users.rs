//! Пользователи и роли.

use axum::extract::State;
use axum::http::StatusCode;
use mw_core::{NewRole, NewUser, Role, RolePatch, User, UserPatch};

use crate::auth::CurrentActor;
use crate::dto::{SetPasswordRequest, VersionParam};
use crate::error::ApiError;
use crate::extract::{Json, Path, Query};
use crate::state::AppState;

/// Список пользователей (право `users_manage`).
#[utoipa::path(
    get,
    path = "/api/v1/users",
    tag = "Пользователи и роли",
    security(("bearer" = [])),
    responses((status = 200, description = "Пользователи", body = Vec<User>))
)]
pub(super) async fn list_users(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
) -> Result<Json<Vec<User>>, ApiError> {
    Ok(Json(state.core.list_users(&actor).await?))
}

/// Новый пользователь (право `users_manage`).
#[utoipa::path(
    post,
    path = "/api/v1/users",
    tag = "Пользователи и роли",
    security(("bearer" = [])),
    request_body = NewUser,
    responses((status = 201, description = "Пользователь создан", body = User))
)]
pub(super) async fn create_user(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Json(new): Json<NewUser>,
) -> Result<(StatusCode, Json<User>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(state.core.create_user(&actor, new).await?),
    ))
}

/// Изменение пользователя по версии записи (право `users_manage`). Правка без различий
/// успешна и версию не проверяет.
#[utoipa::path(
    patch,
    path = "/api/v1/users/{id}",
    tag = "Пользователи и роли",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер пользователя"), VersionParam),
    request_body = UserPatch,
    responses((status = 200, description = "Пользователь после изменения", body = User))
)]
pub(super) async fn update_user(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
    Query(v): Query<VersionParam>,
    Json(patch): Json<UserPatch>,
) -> Result<Json<User>, ApiError> {
    Ok(Json(
        state.core.update_user(&actor, id, v.version, patch).await?,
    ))
}

/// Сброс пароля пользователя администратором (право `users_manage`).
#[utoipa::path(
    put,
    path = "/api/v1/users/{id}/password",
    tag = "Пользователи и роли",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер пользователя")),
    request_body = SetPasswordRequest,
    responses((status = 204, description = "Пароль задан"))
)]
pub(super) async fn set_user_password(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
    Json(body): Json<SetPasswordRequest>,
) -> Result<StatusCode, ApiError> {
    state.core.set_password(&actor, id, &body.password).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Список ролей (право `users_manage`).
#[utoipa::path(
    get,
    path = "/api/v1/roles",
    tag = "Пользователи и роли",
    security(("bearer" = [])),
    responses((status = 200, description = "Роли", body = Vec<Role>))
)]
pub(super) async fn list_roles(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
) -> Result<Json<Vec<Role>>, ApiError> {
    Ok(Json(state.core.list_roles(&actor).await?))
}

/// Новая роль (право `users_manage`). Неизвестные права отбрасываются.
#[utoipa::path(
    post,
    path = "/api/v1/roles",
    tag = "Пользователи и роли",
    security(("bearer" = [])),
    request_body = NewRole,
    responses((status = 201, description = "Роль создана", body = Role))
)]
pub(super) async fn create_role(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Json(new): Json<NewRole>,
) -> Result<(StatusCode, Json<Role>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(state.core.create_role(&actor, new).await?),
    ))
}

/// Изменение роли по версии записи (право `users_manage`). Правка без различий
/// успешна и версию не проверяет.
#[utoipa::path(
    patch,
    path = "/api/v1/roles/{id}",
    tag = "Пользователи и роли",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер роли"), VersionParam),
    request_body = RolePatch,
    responses((status = 200, description = "Роль после изменения", body = Role))
)]
pub(super) async fn update_role(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
    Query(v): Query<VersionParam>,
    Json(patch): Json<RolePatch>,
) -> Result<Json<Role>, ApiError> {
    Ok(Json(
        state.core.update_role(&actor, id, v.version, patch).await?,
    ))
}
