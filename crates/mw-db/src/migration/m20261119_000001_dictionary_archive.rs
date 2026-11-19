//! Архив значений справочников: скрытое значение не предлагается при приёме, но остаётся в старых заказах.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261119_000001_dictionary_archive"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.alter_table(
            Table::alter()
                .table(Alias::new("dictionary_items"))
                .add_column(
                    ColumnDef::new(Alias::new("archived"))
                        .boolean()
                        .not_null()
                        .default(false),
                )
                .to_owned(),
        )
        .await
    }
}
