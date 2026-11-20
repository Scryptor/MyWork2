//! Готовые роли «Мастер» и «Приёмщик». Роль добавляется, только если роли с таким названием
//! (без учёта регистра и «ё») ещё нет; существующие роли не меняются.

use mw_domain::search::normalize_search;
use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261120_000002_builtin_roles"
    }
}

/// (название, права в JSON). Заморожено литералами: смена набора — новая миграция.
const BUILTIN_ROLES: [(&str, &str); 2] = [
    ("Мастер", r#"["orders_edit","clients_view"]"#),
    (
        "Приёмщик",
        r#"["orders_create","orders_edit","clients_view","clients_create","clients_edit"]"#,
    ),
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        let mut select = Query::select();
        select.column(Alias::new("name")).from(Alias::new("roles"));
        let mut taken = Vec::new();
        for row in db.query_all(&select).await? {
            let name: String = row.try_get_by_index(0)?;
            taken.push(normalize_search(&name));
        }
        for (name, permissions) in BUILTIN_ROLES {
            if taken.contains(&normalize_search(name)) {
                continue;
            }
            let mut insert = Query::insert();
            insert
                .into_table(Alias::new("roles"))
                .columns([Alias::new("name"), Alias::new("permissions")])
                .values([name.into(), permissions.into()])
                .map_err(|e| DbErr::Migration(format!("готовая роль «{name}»: {e}")))?;
            m.exec_stmt(insert).await?;
        }
        Ok(())
    }
}
