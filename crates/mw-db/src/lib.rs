//! Доступ к БД MyWork2: схема, подключение, нумерация. Единственное место, где код говорит с СУБД.

pub mod backup;
pub mod connect;
pub mod entity;
pub mod ids;
pub mod migration;
pub mod testing;

pub use connect::{connect, sqlite_url};
pub use migration::{migrate, migrate_up_to};
