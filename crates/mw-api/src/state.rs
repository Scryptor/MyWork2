//! Состояние приложения и настройки HTTP-слоя.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Bytes;
use chrono::Duration;
use mw_core::Core;

use crate::auth::{ACCOUNT_FAILURE_LIMIT, FAILURE_WINDOW_SECS, IP_FAILURE_LIMIT};
use crate::limiter::LoginLimiter;

/// Настройки HTTP-слоя.
#[derive(Debug, Clone, Copy)]
pub struct ApiConfig {
    /// Сервер слушает только loopback-адрес: включает проверку `Host` и режим без входа.
    pub loopback: bool,
}

impl ApiConfig {
    pub fn for_listen_addr(addr: SocketAddr) -> ApiConfig {
        ApiConfig {
            loopback: addr.ip().is_loopback(),
        }
    }
}

/// Общее состояние обработчиков.
#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) core: Core,
    pub(crate) config: ApiConfig,
    /// Готовый документ OpenAPI (JSON), чтобы не собирать его на каждый запрос.
    pub(crate) openapi_json: Bytes,
    /// Ограничители неудачных попыток входа и смены пароля.
    pub(crate) limits: Arc<Limits>,
}

/// Ограничители по учётной записи (логину или пользователю) и по адресу клиента.
pub(crate) struct Limits {
    pub(crate) accounts: LoginLimiter,
    pub(crate) ips: LoginLimiter,
}

impl Limits {
    pub(crate) fn new() -> Self {
        let window = Duration::seconds(FAILURE_WINDOW_SECS);
        Limits {
            accounts: LoginLimiter::new(ACCOUNT_FAILURE_LIMIT, window),
            ips: LoginLimiter::new(IP_FAILURE_LIMIT, window),
        }
    }
}
