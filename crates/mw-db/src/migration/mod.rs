//! Миграции схемы. Строятся построителем SeaQuery, поэтому одинаково работают на SQLite и PostgreSQL.

use sea_orm::{DatabaseConnection, DbErr};
use sea_orm_migration::prelude::*;

mod m20261111_000001_init;
mod m20261112_000001_legacy_import;
mod m20261112_000002_warranty_dictionary;
mod m20261112_000003_default_warranty_text;
mod m20261116_000001_order_keys;
mod m20261117_000001_attachments;
mod m20261119_000001_dictionary_archive;
mod m20261120_000001_user_sign_in_and_master;
mod m20261120_000002_builtin_roles;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20261111_000001_init::Migration),
            Box::new(m20261112_000001_legacy_import::Migration),
            Box::new(m20261112_000002_warranty_dictionary::Migration),
            Box::new(m20261112_000003_default_warranty_text::Migration),
            Box::new(m20261116_000001_order_keys::Migration),
            Box::new(m20261117_000001_attachments::Migration),
            Box::new(m20261119_000001_dictionary_archive::Migration),
            Box::new(m20261120_000001_user_sign_in_and_master::Migration),
            Box::new(m20261120_000002_builtin_roles::Migration),
        ]
    }
}

/// Применяет только первые `steps` миграций (для тестов переноса данных).
pub async fn migrate_up_to(db: &DatabaseConnection, steps: u32) -> Result<(), DbErr> {
    Migrator::up(db, Some(steps)).await
}

/// Применяет все миграции; повторный вызов ничего не делает.
pub async fn migrate(db: &DatabaseConnection) -> Result<(), DbErr> {
    Migrator::up(db, None).await
}
