//! Пользователь без входа (`can_sign_in`) и отметка «Мастер» (`is_master`).
//! Прежние пользователи входят как раньше и мастерами не отмечены.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261120_000001_user_sign_in_and_master"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        // SQLite добавляет за один ALTER TABLE только одну колонку.
        for (column, default) in [("can_sign_in", true), ("is_master", false)] {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("users"))
                    .add_column(
                        ColumnDef::new(Alias::new(column))
                            .boolean()
                            .not_null()
                            .default(default),
                    )
                    .to_owned(),
            )
            .await?;
        }
        Ok(())
    }
}
