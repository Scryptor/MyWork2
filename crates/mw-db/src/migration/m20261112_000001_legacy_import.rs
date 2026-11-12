//! Телефоны клиентов, гарантия текстом, трек-номера, следы импорта из MyWork 2.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261112_000001_legacy_import"
    }
}

fn a(name: &str) -> Alias {
    Alias::new(name)
}

fn id() -> ColumnDef {
    ColumnDef::new(a("id"))
        .big_integer()
        .not_null()
        .auto_increment()
        .primary_key()
        .take()
}

/// Текст без значения: `TEXT NOT NULL DEFAULT ''`.
fn text(name: &str) -> ColumnDef {
    ColumnDef::new(a(name)).text().not_null().default("").take()
}

fn text_required(name: &str) -> ColumnDef {
    ColumnDef::new(a(name)).text().not_null().take()
}

fn text_null(name: &str) -> ColumnDef {
    ColumnDef::new(a(name)).text().null().take()
}

fn int_required(name: &str) -> ColumnDef {
    ColumnDef::new(a(name)).integer().not_null().take()
}

fn big_required(name: &str) -> ColumnDef {
    ColumnDef::new(a(name)).big_integer().not_null().take()
}

fn big_null(name: &str) -> ColumnDef {
    ColumnDef::new(a(name)).big_integer().null().take()
}

fn ts(name: &str) -> ColumnDef {
    ColumnDef::new(a(name)).timestamp().not_null().take()
}

fn fk(
    name: &str,
    from: &str,
    column: &str,
    to: &str,
    action: ForeignKeyAction,
) -> ForeignKeyCreateStatement {
    ForeignKey::create()
        .name(name)
        .from(a(from), a(column))
        .to(a(to), a("id"))
        .on_delete(action)
        .to_owned()
}

fn index(table: &str, columns: &[&str]) -> IndexCreateStatement {
    let mut stmt = Index::create();
    stmt.name(format!("idx_{table}_{}", columns.join("_")))
        .table(a(table));
    for column in columns {
        stmt.col(a(column));
    }
    stmt.to_owned()
}

fn unique(table: &str, columns: &[&str]) -> IndexCreateStatement {
    let mut stmt = Index::create();
    stmt.name(format!("uq_{table}_{}", columns.join("_")))
        .table(a(table))
        .unique();
    for column in columns {
        stmt.col(a(column));
    }
    stmt.to_owned()
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    /// SQLite по умолчанию не оборачивает миграцию в транзакцию; нужна атомарность.
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        use ForeignKeyAction::{Cascade, Restrict};

        m.create_table(
            Table::create()
                .table(a("client_phones"))
                .col(id())
                .col(big_required("client_id"))
                .col(int_required("position"))
                .col(text_required("phone"))
                .col(text_required("phone_key"))
                .col(text("note"))
                .foreign_key(&mut fk(
                    "fk_client_phones_client",
                    "client_phones",
                    "client_id",
                    "clients",
                    Cascade,
                ))
                .to_owned(),
        )
        .await?;
        m.create_index(unique("client_phones", &["client_id", "position"]))
            .await?;
        m.create_index(index("client_phones", &["phone_key"]))
            .await?;

        m.alter_table(
            Table::alter()
                .table(a("orders"))
                .add_column(text("warranty_text"))
                .to_owned(),
        )
        .await?;
        m.alter_table(
            Table::alter()
                .table(a("orders"))
                .add_column(ColumnDef::new(a("warranty_months")).integer().null())
                .to_owned(),
        )
        .await?;

        m.create_table(
            Table::create()
                .table(a("order_tracking"))
                .col(id())
                .col(big_required("order_id"))
                .col(int_required("position"))
                .col(text_required("number"))
                .col(text("description"))
                .foreign_key(&mut fk(
                    "fk_order_tracking_order",
                    "order_tracking",
                    "order_id",
                    "orders",
                    Restrict,
                ))
                .to_owned(),
        )
        .await?;
        m.create_index(unique("order_tracking", &["order_id", "position"]))
            .await?;

        m.create_table(
            Table::create()
                .table(a("legacy_imports"))
                .col(id())
                .col(ts("imported_at"))
                .col(text_required("source_path"))
                .col(text_required("mode"))
                .col(text_required("report"))
                .to_owned(),
        )
        .await?;

        m.create_table(
            Table::create()
                .table(a("legacy_rows"))
                .col(id())
                .col(big_required("import_id"))
                .col(text_required("source"))
                .col(text_required("source_key"))
                .col(text_null("target_table"))
                .col(big_null("target_id"))
                .col(text_required("raw"))
                .foreign_key(&mut fk(
                    "fk_legacy_rows_import",
                    "legacy_rows",
                    "import_id",
                    "legacy_imports",
                    Cascade,
                ))
                .to_owned(),
        )
        .await?;
        m.create_index(unique(
            "legacy_rows",
            &["import_id", "source", "source_key"],
        ))
        .await?;
        m.create_index(index("legacy_rows", &["target_table", "target_id"]))
            .await?;

        // Основной телефон клиента переезжает в `client_phones` (позиция 0).
        let mut select = Query::select();
        select
            .column(a("id"))
            .expr(Expr::val(0))
            .column(a("phone"))
            .column(a("phone_key"))
            .expr(Expr::val(""))
            .from(a("clients"))
            .and_where(Expr::col(a("phone")).ne(""));
        let mut copy = Query::insert();
        copy.into_table(a("client_phones"))
            .columns([
                a("client_id"),
                a("position"),
                a("phone"),
                a("phone_key"),
                a("note"),
            ])
            .select_from(select)
            .map_err(|e| DbErr::Migration(format!("перенос телефонов: {e}")))?;
        m.exec_stmt(copy).await?;

        // Гарантия текстом: 0 дней — «Без гарантии», N > 0 — «N дн.», NULL — пусто.
        let mut none = Query::update();
        none.table(a("orders"))
            .value(a("warranty_text"), "Без гарантии")
            .and_where(Expr::col(a("warranty_days")).eq(0));
        m.exec_stmt(none).await?;
        let mut days = Query::update();
        days.table(a("orders"))
            .value(
                a("warranty_text"),
                Expr::cust("CAST(warranty_days AS TEXT) || ' дн.'"),
            )
            .and_where(Expr::col(a("warranty_days")).gt(0));
        m.exec_stmt(days).await?;
        Ok(())
    }
}
