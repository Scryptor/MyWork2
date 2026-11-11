//! Состояние приложения и настройки HTTP-слоя.

use std::net::SocketAddr;

use axum::body::Bytes;
use mw_core::Core;

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
}
