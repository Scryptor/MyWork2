//! Настройка «гарантия по умолчанию»: число суток (`default_warranty_days`) → текст (`default_warranty`).

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261112_000003_default_warranty_text"
    }
}

const OLD_KEY: &str = "default_warranty_days";
const NEW_KEY: &str = "default_warranty";

/// Текст гарантии из JSON старой настройки: 0 — «Без гарантии», N > 0 — «N дн.», иначе пусто.
fn warranty_text(old_json: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(old_json)
        .ok()
        .and_then(|v| v.as_u64())
    {
        Some(0) => "Без гарантии".to_owned(),
        Some(n) => format!("{n} дн."),
        None => String::new(),
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        let mut select = Query::select();
        select
            .column(Alias::new("value"))
            .from(Alias::new("settings"))
            .and_where(Expr::col(Alias::new("key")).eq(OLD_KEY));
        let Some(row) = db.query_one(&select).await? else {
            return Ok(());
        };
        let old: String = row.try_get_by_index(0)?;
        let new_json = serde_json::Value::String(warranty_text(&old)).to_string();

        let mut insert = Query::insert();
        insert
            .into_table(Alias::new("settings"))
            .columns([Alias::new("key"), Alias::new("value")])
            .values([NEW_KEY.into(), new_json.into()])
            .map_err(|e| DbErr::Migration(format!("настройка гарантии: {e}")))?
            .on_conflict(
                OnConflict::column(Alias::new("key"))
                    .update_column(Alias::new("value"))
                    .to_owned(),
            );
        m.exec_stmt(insert).await?;

        let mut delete = Query::delete();
        delete
            .from_table(Alias::new("settings"))
            .and_where(Expr::col(Alias::new("key")).eq(OLD_KEY));
        m.exec_stmt(delete).await
    }
}
