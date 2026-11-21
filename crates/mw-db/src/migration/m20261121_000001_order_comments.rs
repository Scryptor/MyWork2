//! Отдельные записи ленты заказа. Исходный комментарий заказа сохраняется в его карточке.

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
                .table(a("order_comments"))
                .col(
                    ColumnDef::new(a("id"))
                        .big_integer()
                        .not_null()
                        .auto_increment()
                        .primary_key(),
                )
                .col(ColumnDef::new(a("order_id")).big_integer().not_null())
                .col(ColumnDef::new(a("request_key")).text().not_null())
                .col(ColumnDef::new(a("author_id")).big_integer().null())
                .col(ColumnDef::new(a("author_name")).text().not_null())
                .col(ColumnDef::new(a("body")).text().not_null())
                .col(ColumnDef::new(a("created_at")).timestamp().not_null())
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_order_comments_order")
                        .from(a("order_comments"), a("order_id"))
                        .to(a("orders"), a("id")),
                )
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("uq_order_comments_request_key")
                .table(a("order_comments"))
                .col(a("order_id"))
                .col(a("request_key"))
                .unique()
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("idx_order_comments_order_id_id")
                .table(a("order_comments"))
                .col(a("order_id"))
                .col(a("id"))
                .to_owned(),
        )
        .await
    }
}
