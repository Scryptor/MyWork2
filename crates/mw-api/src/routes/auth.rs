//! Вход, выход, текущий пользователь, смена пароля.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use mw_core::CoreError;
use mw_domain::search::normalize_search;

use crate::auth::{ClientIp, CurrentActor, bearer_token};
use crate::dto::{ChangePasswordRequest, CurrentUser, LoginRequest, LoginResponse};
use crate::error::ApiError;
use crate::extract::Json;
use crate::state::AppState;

/// Наибольшая часть логина в ключе ограничителя (длинные логины не раздувают память).
const LOGIN_KEY_CHARS: usize = 64;

/// Блокировка по любому из ключей: берётся наибольший срок.
fn check_limits(state: &AppState, account_key: Option<&str>, ip_key: &str) -> Result<(), ApiError> {
    let now = state.core.now_utc();
    let account = match account_key {
        Some(key) => state.limits.accounts.check(key, now),
        None => Ok(()),
    };
    let ip = state.limits.ips.check(ip_key, now);
    match (account, ip) {
        (Ok(()), Ok(())) => Ok(()),
        (a, b) => {
            let secs = a.err().into_iter().chain(b.err()).max().unwrap_or(1);
            Err(ApiError::too_many_attempts(secs))
        }
    }
}

fn record_failure(state: &AppState, account_key: Option<&str>, ip_key: &str) {
    let now = state.core.now_utc();
    if let Some(key) = account_key {
        state.limits.accounts.record_failure(key, now);
    }
    state.limits.ips.record_failure(ip_key, now);
}

/// Вход по логину и паролю. Выдаёт токен для заголовка `Authorization: Bearer`.
///
/// После 5 неудач для одного логина или 20 неудач с одного адреса за 15 минут вход
/// блокируется (429 с `Retry-After`); пока блокировка действует, пароль не проверяется.
#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    tag = "Вход",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Вход выполнен", body = LoginResponse),
        (status = 401, description = "Неверный логин или пароль", body = crate::ErrorBody),
        (status = 429, description = "Слишком много неудачных попыток", body = crate::ErrorBody)
    )
)]
pub(super) async fn login(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    let normalized = normalize_search(&req.login);
    let login_key = format!(
        "login:{}",
        normalized.chars().take(LOGIN_KEY_CHARS).collect::<String>()
    );
    let ip_key = format!("ip:{ip}");
    check_limits(&state, Some(&login_key), &ip_key)?;
    match state
        .core
        .login(&req.login, &req.password, &req.device_label)
        .await
    {
        Ok(session) => {
            state.limits.accounts.reset(&login_key);
            Ok(Json(LoginResponse {
                token: session.token,
                user: session.user,
                permissions: session.permissions,
            }))
        }
        Err(CoreError::InvalidCredentials) => {
            record_failure(&state, Some(&login_key), &ip_key);
            Err(CoreError::InvalidCredentials.into())
        }
        Err(e) => Err(e.into()),
    }
}

/// Выход: сеанс с токеном из заголовка закрывается. Без токена ничего не делает; ответ всегда 204.
#[utoipa::path(
    post,
    path = "/api/v1/auth/logout",
    tag = "Вход",
    security(("bearer" = [])),
    responses((status = 204, description = "Готово"))
)]
pub(super) async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    if let Ok(Some(token)) = bearer_token(&headers) {
        state.core.logout(token).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Кто сейчас делает запросы и какие у него права.
#[utoipa::path(
    get,
    path = "/api/v1/auth/me",
    tag = "Вход",
    security(("bearer" = [])),
    responses((status = 200, description = "Текущий пользователь", body = CurrentUser))
)]
pub(super) async fn me(CurrentActor(actor): CurrentActor) -> Json<CurrentUser> {
    Json(CurrentUser {
        user_id: actor.user_id(),
        name: actor.name().to_owned(),
        permissions: actor.permissions().clone(),
        open_mode: actor.user_id().is_none(),
    })
}

/// Смена собственного пароля. Все сеансы пользователя закрываются, нужен новый вход.
///
/// Неверный текущий пароль считается неудачной попыткой: после 5 неудач за 15 минут
/// (или 20 с одного адреса) приходит 429 с `Retry-After`.
#[utoipa::path(
    post,
    path = "/api/v1/auth/password",
    tag = "Вход",
    security(("bearer" = [])),
    request_body = ChangePasswordRequest,
    responses(
        (status = 204, description = "Пароль изменён"),
        (status = 422, description = "Неверный текущий или слишком короткий новый пароль", body = crate::ErrorBody),
        (status = 429, description = "Слишком много неудачных попыток", body = crate::ErrorBody)
    )
)]
pub(super) async fn change_password(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    ClientIp(ip): ClientIp,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<StatusCode, ApiError> {
    let user_key = actor.user_id().map(|id| format!("user:{id}"));
    let ip_key = format!("ip:{ip}");
    check_limits(&state, user_key.as_deref(), &ip_key)?;
    match state
        .core
        .change_own_password(&actor, &req.current_password, &req.new_password)
        .await
    {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(CoreError::Validation(fields)) => {
            if fields.iter().any(|f| f.field == "current_password") {
                record_failure(&state, user_key.as_deref(), &ip_key);
            }
            Err(CoreError::Validation(fields).into())
        }
        Err(e) => Err(e.into()),
    }
}
