//! Миграции схемы. Строятся построителем SeaQuery, поэтому одинаково работают на SQLite и PostgreSQL.

use sea_orm::{DatabaseConnection, DbErr};
use sea_orm_migration::prelude::*;

mod m20261111_000001_init;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m20261111_000001_init::Migration)]
    }
}

/// Применяет все миграции; повторный вызов ничего не делает.
pub async fn migrate(db: &DatabaseConnection) -> Result<(), DbErr> {
    Migrator::up(db, None).await
}
