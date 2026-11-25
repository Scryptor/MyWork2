//! Право «Выгрузка» (`data_export`): получают роли, у которых были все 17 прав до него, — то есть
//! администраторы под любым названием. Роль с неполным набором (готовые «Мастер» и «Приёмщик»,
//! роль без журнала действий) остаётся как есть: выдать право можно в настройках.

use std::collections::BTreeSet;

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261125_000003_data_export_permission"
    }
}

fn a(name: &str) -> Alias {
    Alias::new(name)
}

// Данные ЗАМОРОЖЕНЫ литералами: правка `Permission` в `mw-domain` выпущенную миграцию не меняет.

/// Все 17 прав до выгрузки — копия `SEED_ADMIN_PERMISSIONS` из `m20261111_000001_init`.
const PERMISSIONS_BEFORE: &str = r#"["orders_create","orders_edit","orders_delete","order_dates_edit","reports_view","stock_view","stock_create","stock_edit","stock_delete","clients_view","clients_create","clients_edit","clients_delete","clients_merge","settings_manage","users_manage","audit_view"]"#;

const NEW_PERMISSION: &str = "data_export";
/// Право, после которого оно стоит в списке (порядок `Permission`).
const AFTER: &str = "reports_view";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let before: BTreeSet<String> = serde_json::from_str(PERMISSIONS_BEFORE)
            .map_err(|e| DbErr::Migration(format!("права до выгрузки: {e}")))?;
        let db = m.get_connection();
        let mut select = Query::select();
        select
            .columns([a("id"), a("permissions")])
            .from(a("roles"))
            .order_by(a("id"), Order::Asc);
        for row in db.query_all(&select).await? {
            let id: i64 = row.try_get_by_index(0)?;
            let permissions: String = row.try_get_by_index(1)?;
            // Не JSON-массив строк — не наша роль.
            let Ok(mut list) = serde_json::from_str::<Vec<String>>(&permissions) else {
                continue;
            };
            let set: BTreeSet<&String> = list.iter().collect();
            if list.iter().any(|p| p == NEW_PERMISSION) || !before.iter().all(|p| set.contains(p)) {
                continue;
            }
            let at = list
                .iter()
                .position(|p| p == AFTER)
                .map_or(list.len(), |i| i + 1);
            list.insert(at, NEW_PERMISSION.to_owned());
            let json = serde_json::to_string(&list)
                .map_err(|e| DbErr::Migration(format!("права роли: {e}")))?;
            let mut update = Query::update();
            update
                .table(a("roles"))
                .value(a("permissions"), json)
                .value(a("version"), Expr::col(a("version")).add(1))
                .and_where(Expr::col(a("id")).eq(id));
            db.execute(&update).await?;
        }
        Ok(())
    }
}
