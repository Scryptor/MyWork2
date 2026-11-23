//! Шаблоны печатных документов, изменённые предприятием. Нет строки или `model IS NULL` — стандартный.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let a = Alias::new;
        m.create_table(
            Table::create()
                .table(a("document_templates"))
                .col(ColumnDef::new(a("kind")).text().not_null().primary_key())
                .col(ColumnDef::new(a("model")).text().null())
                .col(ColumnDef::new(a("version")).big_integer().not_null())
                .col(ColumnDef::new(a("updated_at")).timestamp().not_null())
                .to_owned(),
        )
        .await
    }
}
