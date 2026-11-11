//! Типы самого HTTP-слоя.

use std::fmt;

use mw_core::User;
use mw_domain::permission::PermissionSet;
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

/// Вход в программу.
#[derive(Clone, Deserialize, TS, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct LoginRequest {
    /// Логин; регистр и крайние пробелы не важны.
    pub login: String,
    /// Пароль.
    pub password: String,
    /// Название устройства для списка сеансов (например, «Касса»); может быть пустым.
    pub device_label: String,
}

impl fmt::Debug for LoginRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoginRequest")
            .field("login", &self.login)
            .field("password", &"***")
            .field("device_label", &self.device_label)
            .finish()
    }
}

/// Результат входа. Токен показывается только один раз.
#[derive(Clone, Serialize, Deserialize, TS, ToSchema)]
pub struct LoginResponse {
    /// Токен для заголовка `Authorization: Bearer <токен>` (64 hex-символа).
    pub token: String,
    /// Пользователь.
    pub user: User,
    /// Права пользователя.
    pub permissions: PermissionSet,
}

impl fmt::Debug for LoginResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoginResponse")
            .field("token", &"***")
            .field("user", &self.user)
            .field("permissions", &self.permissions)
            .finish()
    }
}

/// Кто делает запросы.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
pub struct CurrentUser {
    /// Номер пользователя; пусто в режиме без входа.
    pub user_id: Option<i64>,
    /// Имя для показа и журнала действий.
    pub name: String,
    /// Права.
    pub permissions: PermissionSet,
    /// Работа без входа (сервер на локальном адресе, пользователей нет).
    pub open_mode: bool,
}

/// Смена собственного пароля.
#[derive(Clone, Deserialize, TS, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangePasswordRequest {
    /// Текущий пароль.
    pub current_password: String,
    /// Новый пароль (не короче `MIN_PASSWORD_CHARS` символов).
    pub new_password: String,
}

impl fmt::Debug for ChangePasswordRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChangePasswordRequest")
            .field("current_password", &"***")
            .field("new_password", &"***")
            .finish()
    }
}
