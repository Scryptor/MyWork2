//! Кто делает запрос: токен из заголовка, режим без входа, адрес клиента.

use std::net::{IpAddr, SocketAddr};

use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::HeaderMap;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use mw_core::{Actor, CoreError};

use crate::error::ApiError;
use crate::state::AppState;

/// Имя «пользователя» в журнале действий в режиме без входа.
pub const OPEN_MODE_ACTOR: &str = "Без входа";
/// Неудач на одну учётную запись за окно, после которых вход блокируется.
pub const ACCOUNT_FAILURE_LIMIT: u32 = 5;
/// Неудач с одного адреса за окно, после которых вход с него блокируется.
pub const IP_FAILURE_LIMIT: u32 = 20;
/// Длина окна подсчёта неудач, секунды.
pub const FAILURE_WINDOW_SECS: i64 = 900;

/// Токен из заголовка `Authorization: Bearer <токен>`.
/// `Ok(None)` — заголовка нет; заголовок другого вида — `unauthenticated`.
pub(crate) fn bearer_token(headers: &HeaderMap) -> Result<Option<&str>, ApiError> {
    let Some(value) = headers.get(AUTHORIZATION) else {
        return Ok(None);
    };
    let text = value
        .to_str()
        .map_err(|_| ApiError::from(CoreError::Unauthenticated))?;
    match text.split_once(' ') {
        Some((scheme, token)) if scheme.eq_ignore_ascii_case("Bearer") => Ok(Some(token)),
        _ => Err(CoreError::Unauthenticated.into()),
    }
}

/// Пользователь запроса: по токену или, в режиме без входа, `Actor` с полными правами.
pub(crate) struct CurrentActor(pub(crate) Actor);

impl FromRequestParts<AppState> for CurrentActor {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        match bearer_token(&parts.headers)? {
            Some(token) => Ok(CurrentActor(state.core.authenticate(token).await?)),
            None => {
                if state.config.loopback() && state.core.is_open_mode().await? {
                    Ok(CurrentActor(Actor::unrestricted(OPEN_MODE_ACTOR)))
                } else {
                    Err(CoreError::Unauthenticated.into())
                }
            }
        }
    }
}

/// Адрес клиента из `ConnectInfo`.
pub(crate) struct ClientIp(pub(crate) IpAddr);

impl<S: Send + Sync> FromRequestParts<S> for ClientIp {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        match parts.extensions.get::<ConnectInfo<SocketAddr>>() {
            Some(ConnectInfo(addr)) => Ok(ClientIp(addr.ip())),
            None => Err(CoreError::Internal("нет адреса клиента в запросе".into()).into()),
        }
    }
}
