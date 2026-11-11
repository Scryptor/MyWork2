//! Заказы: приём устройства и чтение.

mod create;
mod types;
mod update;

pub use types::{ClientRef, NewOrder, Order, OrderPatch};
