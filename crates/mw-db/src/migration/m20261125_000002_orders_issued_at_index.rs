//! Индекс по `orders.issued_at`: отчёты берут заказы по дню выдачи.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261125_000002_orders_issued_at_index"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.create_index(
            Index::create()
                .name("idx_orders_issued_at")
                .table(Alias::new("orders"))
                .col(Alias::new("issued_at"))
                .to_owned(),
        )
        .await
    }
}
