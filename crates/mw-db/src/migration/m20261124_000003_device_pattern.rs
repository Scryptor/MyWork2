//! Графический ключ устройства: точки сетки 3 × 3 строкой цифр 1–9 в порядке ввода («1589»),
//! пусто — ключа нет. Хранится, как пароль устройства, открытым текстом.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261124_000003_device_pattern"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.alter_table(
            Table::alter()
                .table(Alias::new("orders"))
                .add_column(
                    ColumnDef::new(Alias::new("device_pattern"))
                        .text()
                        .not_null()
                        .default(""),
                )
                .to_owned(),
        )
        .await
    }
}
