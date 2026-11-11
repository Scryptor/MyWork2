//! Пароли, пользователи и роли. Сессии и вход — отдельный слой поверх этого модуля.

mod password;
mod roles;
mod users;

pub use password::{PasswordScheme, hash_password, legacy_sha1_hex, verify_password};
pub use roles::{NewRole, Role, RolePatch};
pub use users::{NewUser, User, UserPatch};

/// Минимальная длина пароля в символах (не в байтах).
pub const MIN_PASSWORD_CHARS: usize = 4;
