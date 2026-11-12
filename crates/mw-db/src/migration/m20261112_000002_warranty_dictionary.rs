//! Справочник сроков гарантии (`warranty`): стартовые значения.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261112_000002_warranty_dictionary"
    }
}

/// Значения заморожены литералами: изменение констант `mw-domain` не меняет выпущенную миграцию.
const WARRANTY: &[&str] = &[
    "Без гарантии",
    "14 дней",
    "30 дней",
    "3 месяца",
    "6 месяцев",
    "1 год",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        for (order, value) in (0_i32..).zip(WARRANTY.iter()) {
            let mut insert = Query::insert();
            insert
                .into_table(Alias::new("dictionary_items"))
                .columns([
                    Alias::new("kind"),
                    Alias::new("value"),
                    Alias::new("sort_order"),
                ])
                .values(["warranty".into(), (*value).into(), order.into()])
                .map_err(|e| DbErr::Migration(format!("стартовые данные: {e}")))?;
            m.exec_stmt(insert).await?;
        }
        Ok(())
    }
}
