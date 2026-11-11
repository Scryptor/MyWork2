//! Прикладной слой MyWork2: бизнес-правила поверх `mw-db`. Все операции — методы `Core`.

mod actor;
mod audit;
mod clock;
mod core;
mod dictionaries;
mod error;
mod page;
mod service_points;
mod settings;
mod statuses;

pub use actor::Actor;
pub use clock::{Clock, FixedClock, SystemClock};
pub use core::Core;
pub use dictionaries::DictionaryItem;
pub use error::CoreError;
pub use page::Page;
pub use service_points::ServicePoint;
pub use settings::{AppSettings, SettingsPatch};
pub use statuses::{NewStatus, Status, StatusPatch};

pub(crate) use audit::{NewAuditEntry, write_audit};
