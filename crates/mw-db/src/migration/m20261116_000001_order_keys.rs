//! Ключи сортировки и фильтров заказа (нормализованные тип, бренд, модель, мастер) и время
//! последней смены статуса. Ключи заполняет переиндексация при открытии `Core`.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261116_000001_order_keys"
    }
}

const KEYS: [&str; 4] = ["device_type_key", "brand_key", "model_key", "master_key"];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let orders = Alias::new("orders");
        m.alter_table(
            Table::alter()
                .table(orders.clone())
                .add_column(
                    ColumnDef::new(Alias::new("status_changed_at"))
                        .timestamp()
                        .null(),
                )
                .to_owned(),
        )
        .await?;
        for key in KEYS {
            m.alter_table(
                Table::alter()
                    .table(orders.clone())
                    .add_column(
                        ColumnDef::new(Alias::new(key))
                            .text()
                            .not_null()
                            .default(""),
                    )
                    .to_owned(),
            )
            .await?;
        }
        for key in ["brand_key", "master_key", "device_type_key"] {
            m.create_index(
                Index::create()
                    .name(format!("idx_orders_{key}"))
                    .table(orders.clone())
                    .col(Alias::new(key))
                    .to_owned(),
            )
            .await?;
        }
        crate::backfill::status_changed_at(m.get_connection()).await?;
        Ok(())
    }
}
