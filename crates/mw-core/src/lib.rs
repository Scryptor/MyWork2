//! Прикладной слой MyWork2: бизнес-правила поверх `mw-db`. Все операции — методы `Core`.

mod actor;
mod audit;
mod clock;
mod core;
mod error;
mod page;
mod settings;

pub use actor::Actor;
pub use clock::{Clock, FixedClock, SystemClock};
pub use core::Core;
pub use error::CoreError;
pub use page::Page;
pub use settings::{AppSettings, SettingsPatch};

pub(crate) use audit::{NewAuditEntry, write_audit};
