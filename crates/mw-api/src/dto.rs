//! Типы самого HTTP-слоя.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use utoipa::ToSchema;

/// Сведения о сервере; доступны без входа.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
pub struct ServerInfo {
    /// Всегда `mw`.
    pub app: String,
    /// Версия сервера.
    pub version: String,
    /// Версия API (путь `/api/v{N}`).
    pub api_version: u32,
    /// Нужен ли вход: `false` только в режиме без входа на локальном адресе.
    pub auth_required: bool,
    /// Есть ли в базе активные пользователи.
    pub has_users: bool,
}
