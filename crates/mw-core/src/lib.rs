//! Прикладной слой MyWork2: бизнес-правила поверх `mw-db`. Все операции — методы `Core`.

mod actor;
pub mod attachments;
mod audit;
mod auth;
mod clients;
mod clients_merge;
mod clock;
mod core;
mod dictionaries;
mod error;
mod import;
mod orders;
mod page;
mod patch;
mod search_index;
mod service_points;
mod settings;
mod sql;
mod statuses;
mod usage;

pub use actor::Actor;
pub use attachments::{Attachment, AttachmentFile, AttachmentKind, AttachmentVariant};
pub use audit::{AuditEntry, AuditFilter};
pub use auth::{
    LAST_SEEN_THROTTLE_SECS, MAX_CONCURRENT_PASSWORD_HASHES, MIN_PASSWORD_CHARS, NewRole, NewUser,
    PasswordScheme, Role, RolePatch, SESSION_IDLE_DAYS, Session, User, UserPatch, hash_password,
    legacy_sha1_hex, verify_password,
};
pub use clients::{
    Client, ClientFilter, ClientListItem, ClientPatch, ClientPhone, ClientSort, ClientSortField,
    MAX_CLIENT_PHONES, NewClient,
};
pub use clients_merge::{MAX_MERGE_SOURCES, MAX_SIMILAR_CLIENTS, MergeClients, VersionedId};
pub use clock::{Clock, FixedClock, SystemClock};
pub use core::Core;
pub use dictionaries::{DictionaryItem, DictionaryItemPatch};
pub use error::CoreError;
pub use import::{
    DatabaseKind, ImportBundle, ImportMode, ImportSummary, ImportTarget, ImportedClient,
    ImportedHistory, ImportedLegacyRow, ImportedOrder, ImportedSettings, ImportedUser,
    LegacyTarget,
};
pub use orders::{
    ClientRef, IssuedFilter, NewOrder, NewWarrantyCase, Order, OrderFilter, OrderListItem,
    OrderPatch, OrderSort, OrderSortField, OrderSummary, OrderValueField, StatusCount,
};
pub use page::{Page, PageRequest};
pub use search_index::{ReindexStats, SEARCH_INDEX_VERSION};
pub use service_points::{ServicePoint, ServicePointPatch};
pub use settings::{AppSettings, SettingsPatch};
pub use statuses::{NewStatus, Status, StatusPatch};
pub use usage::CatalogUsage;

pub(crate) use audit::{NewAuditEntry, write_audit};
