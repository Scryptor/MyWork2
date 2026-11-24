//! Склад: позиции (`stock_items`), запчасти заказов (`order_parts`), фото позиции
//! (`attachments.stock_item_id`) и журнал позиции (`audit_log.stock_item_id`).

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261124_000001_stock"
    }
}

fn a(name: &str) -> Alias {
    Alias::new(name)
}

/// `BIGINT` с автоинкрементом (в SQLite — `AUTOINCREMENT`: следующий номер живёт в
/// `sqlite_sequence`, его выставляет `ids::set_next_id`).
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

/// `BIGINT NOT NULL DEFAULT 0`.
fn number(name: &str) -> ColumnDef {
    ColumnDef::new(a(name))
        .big_integer()
        .not_null()
        .default(0)
        .take()
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
        let mut items = Table::create();
        items.table(a("stock_items")).col(id());
        for name in [
            "name",
            "device_type",
            "part_type",
            "brand",
            "model",
            "color",
            "note",
        ] {
            items.col(text(name));
        }
        items
            .col(number("quantity"))
            .col(number("price"))
            .col(number("restock_threshold"))
            .col(text("search_text"))
            .col(text("name_key"))
            .col(ColumnDef::new(a("created_at")).timestamp().not_null())
            .col(ColumnDef::new(a("updated_at")).timestamp().not_null())
            .col(ColumnDef::new(a("deleted_at")).timestamp().null())
            .col(
                ColumnDef::new(a("version"))
                    .big_integer()
                    .not_null()
                    .default(1),
            );
        m.create_table(items.to_owned()).await?;
        m.create_index(index("stock_items", "name_key")).await?;

        // `added_by_id` без внешнего ключа, как `order_comments.author_id`: имя остаётся снимком.
        m.create_table(
            Table::create()
                .table(a("order_parts"))
                .col(id())
                .col(ColumnDef::new(a("order_id")).big_integer().not_null())
                .col(ColumnDef::new(a("stock_item_id")).big_integer().null())
                .col(ColumnDef::new(a("name")).text().not_null())
                .col(ColumnDef::new(a("price")).big_integer().not_null())
                .col(ColumnDef::new(a("quantity")).big_integer().not_null())
                .col(ColumnDef::new(a("added_by_id")).big_integer().null())
                .col(ColumnDef::new(a("added_by_name")).text().not_null())
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
                        .name("fk_order_parts_order")
                        .from(a("order_parts"), a("order_id"))
                        .to(a("orders"), a("id")),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_order_parts_stock_item")
                        .from(a("order_parts"), a("stock_item_id"))
                        .to(a("stock_items"), a("id")),
                )
                .to_owned(),
        )
        .await?;
        // NULL у перенесённых не мешает уникальности: NULL не равен NULL на обоих движках.
        m.create_index(
            Index::create()
                .name("uq_order_parts_request_key")
                .table(a("order_parts"))
                .col(a("order_id"))
                .col(a("request_key"))
                .unique()
                .to_owned(),
        )
        .await?;
        for column in ["order_id", "stock_item_id"] {
            m.create_index(index("order_parts", column)).await?;
        }

        // Без внешних ключей, как `order_id` в этих же таблицах.
        for table in ["attachments", "audit_log"] {
            m.alter_table(
                Table::alter()
                    .table(a(table))
                    .add_column(ColumnDef::new(a("stock_item_id")).big_integer().null())
                    .to_owned(),
            )
            .await?;
            m.create_index(index(table, "stock_item_id")).await?;
        }
        Ok(())
    }
}
