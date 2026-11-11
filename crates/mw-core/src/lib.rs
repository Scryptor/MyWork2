//! Прикладной слой MyWork2: бизнес-правила поверх `mw-db`. Все операции — методы `Core`.

mod actor;
mod audit;
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
pub use service_points::ServicePoint;
pub use settings::{AppSettings, SettingsPatch};
pub use statuses::{NewStatus, Status, StatusPatch};

pub(crate) use audit::{NewAuditEntry, write_audit};
