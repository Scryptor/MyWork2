//! Вложения заказов (фото и файлы), ключ повтора приёма заказа и ключ имени клиента;
//! файлы старой программы становятся вложениями.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261117_000001_attachments"
    }
}

fn a(name: &str) -> Alias {
    Alias::new(name)
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.create_table(
            Table::create()
                .table(a("attachments"))
                .col(
                    ColumnDef::new(a("id"))
                        .big_integer()
                        .not_null()
                        .auto_increment()
                        .primary_key(),
                )
                .col(ColumnDef::new(a("order_id")).big_integer().null())
                .col(ColumnDef::new(a("draft_key")).text().null())
                .col(ColumnDef::new(a("kind")).text().not_null())
                .col(ColumnDef::new(a("name")).text().not_null())
                .col(ColumnDef::new(a("caption")).text().not_null().default(""))
                .col(ColumnDef::new(a("content_type")).text().not_null())
                .col(ColumnDef::new(a("size")).big_integer().not_null())
                .col(ColumnDef::new(a("width")).integer().null())
                .col(ColumnDef::new(a("height")).integer().null())
                .col(ColumnDef::new(a("path")).text().not_null())
                .col(ColumnDef::new(a("thumb_path")).text().null())
                .col(
                    ColumnDef::new(a("legacy"))
                        .boolean()
                        .not_null()
                        .default(false),
                )
                .col(ColumnDef::new(a("created_at")).timestamp().not_null())
                .to_owned(),
        )
        .await?;
        for column in ["order_id", "draft_key"] {
            m.create_index(
                Index::create()
                    .name(format!("idx_attachments_{column}"))
                    .table(a("attachments"))
                    .col(a(column))
                    .to_owned(),
            )
            .await?;
        }

        m.alter_table(
            Table::alter()
                .table(a("orders"))
                .add_column(ColumnDef::new(a("request_key")).text().null())
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("uq_orders_request_key")
                .table(a("orders"))
                .col(a("request_key"))
                .unique()
                .to_owned(),
        )
        .await?;

        m.alter_table(
            Table::alter()
                .table(a("clients"))
                .add_column(ColumnDef::new(a("name_key")).text().not_null().default(""))
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("idx_clients_name_key")
                .table(a("clients"))
                .col(a("name_key"))
                .to_owned(),
        )
        .await?;

        crate::backfill::legacy_client_files(m.get_connection()).await?;
        Ok(())
    }
}
