//! Платежи заказов (`order_payments`).

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261126_000001_order_payments"
    }
}

fn a(name: &str) -> Alias {
    Alias::new(name)
}

/// `BIGINT` с автоинкрементом (в SQLite — `AUTOINCREMENT`).
fn id() -> ColumnDef {
    ColumnDef::new(a("id"))
        .big_integer()
        .not_null()
        .auto_increment()
        .primary_key()
        .take()
}

/// `TEXT NOT NULL DEFAULT ''`.
fn text(name: &str) -> ColumnDef {
    ColumnDef::new(a(name)).text().not_null().default("").take()
}

fn index(table: &str, column: &str) -> IndexCreateStatement {
    Index::create()
        .name(format!("idx_{table}_{column}"))
        .table(a(table))
        .col(a(column))
        .to_owned()
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        // `added_by_id` без внешнего ключа, как у `order_parts`: имя остаётся снимком.
        m.create_table(
            Table::create()
                .table(a("order_payments"))
                .col(id())
                .col(ColumnDef::new(a("order_id")).big_integer().not_null())
                .col(ColumnDef::new(a("day")).date().not_null())
                .col(ColumnDef::new(a("amount")).big_integer().not_null())
                .col(text("received_by"))
                .col(ColumnDef::new(a("stage")).text().not_null())
                .col(ColumnDef::new(a("added_by_id")).big_integer().null())
                .col(text("added_by_name"))
                .col(ColumnDef::new(a("added_at")).timestamp().not_null())
                .col(
                    ColumnDef::new(a("legacy"))
                        .boolean()
                        .not_null()
                        .default(false),
                )
                .col(ColumnDef::new(a("request_key")).text().null())
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_order_payments_order")
                        .from(a("order_payments"), a("order_id"))
                        .to(a("orders"), a("id")),
                )
                .to_owned(),
        )
        .await?;
        // NULL у перенесённых не мешает уникальности: NULL не равен NULL на обоих движках.
        m.create_index(
            Index::create()
                .name("uq_order_payments_request_key")
                .table(a("order_payments"))
                .col(a("order_id"))
                .col(a("request_key"))
                .unique()
                .to_owned(),
        )
        .await?;
        m.create_index(index("order_payments", "order_id")).await
    }
}
