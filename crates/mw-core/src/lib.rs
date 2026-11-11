//! Прикладной слой MyWork2: бизнес-правила поверх `mw-db`. Все операции — методы `Core`.

mod actor;
mod audit;
mod auth;
mod clients;
mod clock;
mod core;
mod dictionaries;
mod error;
mod orders;
mod page;
mod service_points;
mod settings;
mod sql;
mod statuses;

pub use actor::Actor;
pub use audit::{AuditEntry, AuditFilter};
pub use auth::{
    LAST_SEEN_THROTTLE_SECS, MAX_CONCURRENT_PASSWORD_HASHES, MIN_PASSWORD_CHARS, NewRole, NewUser,
    PasswordScheme, Role, RolePatch, SESSION_IDLE_DAYS, Session, User, UserPatch, hash_password,
    legacy_sha1_hex, verify_password,
};
pub use clients::{Client, ClientPatch, NewClient};
pub use clock::{Clock, FixedClock, SystemClock};
pub use core::Core;
pub use dictionaries::DictionaryItem;
pub use error::CoreError;
pub use orders::{
    ClientRef, IssuedFilter, NewOrder, Order, OrderFilter, OrderListItem, OrderPatch, OrderSort,
    OrderSortField,
};
pub use page::Page;
pub use service_points::{ServicePoint, ServicePointPatch};
pub use settings::{AppSettings, SettingsPatch};
pub use statuses::{NewStatus, Status, StatusPatch};

pub(crate) use audit::{NewAuditEntry, write_audit};
