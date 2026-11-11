//! Заказы: приём устройства и чтение.

mod create;
mod types;

pub use types::{ClientRef, NewOrder, Order};

// Используется задачей 11.
#[allow(unused_imports)]
pub(crate) use types::order_search_text;
