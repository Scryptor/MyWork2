//! Пароли, пользователи и роли. Сессии и вход — отдельный слой поверх этого модуля.

mod password;
mod roles;
mod sessions;
mod users;

pub(crate) use password::HashGate;
pub use password::{PasswordScheme, hash_password, legacy_sha1_hex, verify_password};
pub use roles::{NewRole, Role, RolePatch};
pub use sessions::{LAST_SEEN_THROTTLE_SECS, SESSION_IDLE_DAYS, Session};
pub use users::{NewUser, User, UserPatch};

/// Минимальная длина пароля в символах (не в байтах).
pub const MIN_PASSWORD_CHARS: usize = 4;

/// Сколько вычислений Argon2 ядро выполняет одновременно; остальные ждут своей очереди.
pub const MAX_CONCURRENT_PASSWORD_HASHES: usize = 4;
