//! Карта склада: шкафы (`stock_racks`) и места позиций в них (`stock_places`). Каскадов нет:
//! занятый шкаф и позиция с местами не удаляются без явных действий ядра.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261125_000004_stock_places"
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

/// `INTEGER NOT NULL`.
fn number(name: &str) -> ColumnDef {
    ColumnDef::new(a(name)).integer().not_null().take()
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
        m.create_table(
            Table::create()
                .table(a("stock_racks"))
                .col(id())
                .col(ColumnDef::new(a("code")).text().not_null())
                .col(text("label"))
                .col(number("shelves"))
                .col(number("cells_per_shelf"))
                .col(ColumnDef::new(a("service_point_id")).big_integer().null())
                .col(
                    ColumnDef::new(a("sort_order"))
                        .integer()
                        .not_null()
                        .default(0),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_stock_racks_service_point")
                        .from(a("stock_racks"), a("service_point_id"))
                        .to(a("service_points"), a("id")),
                )
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("uq_stock_racks_code")
                .table(a("stock_racks"))
                .col(a("code"))
                .unique()
                .to_owned(),
        )
        .await?;

        m.create_table(
            Table::create()
                .table(a("stock_places"))
                .col(id())
                .col(ColumnDef::new(a("stock_item_id")).big_integer().not_null())
                .col(ColumnDef::new(a("rack_id")).big_integer().not_null())
                .col(number("shelf"))
                .col(number("cell"))
                .col(ColumnDef::new(a("quantity")).big_integer().not_null())
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_stock_places_stock_item")
                        .from(a("stock_places"), a("stock_item_id"))
                        .to(a("stock_items"), a("id")),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_stock_places_rack")
                        .from(a("stock_places"), a("rack_id"))
                        .to(a("stock_racks"), a("id")),
                )
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("uq_stock_places_item_cell")
                .table(a("stock_places"))
                .col(a("stock_item_id"))
                .col(a("rack_id"))
                .col(a("shelf"))
                .col(a("cell"))
                .unique()
                .to_owned(),
        )
        .await?;
        m.create_index(index("stock_places", "rack_id")).await?;
        Ok(())
    }
}
