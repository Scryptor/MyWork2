//! Мастер заказа — пользователь: `orders.master_user_id` (ссылка на `users.id`). Имя мастера
//! текстом (`orders.master`) остаётся как было — снимок для истории и старых заказов.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261120_000003_order_master_user"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let orders = Alias::new("orders");
        // SQLite не добавляет внешний ключ отдельной командой: ссылка — в описании колонки
        // (у такой колонки значение по умолчанию обязано быть NULL).
        m.alter_table(
            Table::alter()
                .table(orders.clone())
                .add_column(
                    ColumnDef::new(Alias::new("master_user_id"))
                        .big_integer()
                        .null()
                        .extra("REFERENCES users (id) ON DELETE RESTRICT"),
                )
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("idx_orders_master_user_id")
                .table(orders)
                .col(Alias::new("master_user_id"))
                .to_owned(),
        )
        .await
    }
}
