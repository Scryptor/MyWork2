//! Стартовые данные склада: справочник «Типы запчастей» и право «Просмотр склада» у готовых ролей
//! «Мастер» и «Приёмщик» — только если права роли не меняли (совпадают с правами
//! `m20261120_000002_builtin_roles` как множество). Роль — по названию без учёта регистра и «ё»,
//! как в той миграции; изменённая роль остаётся как есть.

use std::collections::BTreeSet;

use mw_domain::search::normalize_search;
use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261124_000002_stock_data"
    }
}

fn a(name: &str) -> Alias {
    Alias::new(name)
}

// Данные ЗАМОРОЖЕНЫ литералами: правка констант `mw-domain` и `crate::roles` выпущенную миграцию
// не меняет.

/// Значения справочника «Типы запчастей»; порядок = `sort_order`.
const PART_TYPES: &[&str] = &[
    "Дисплей",
    "Аккумулятор",
    "Разъём зарядки",
    "Шлейф",
    "Камера",
    "Динамик",
    "Микрофон",
    "Корпус",
    "Стекло",
    "Кнопка",
    "Плата",
];

/// (название, права до склада — литерал `m20261120_000002_builtin_roles`, права со складом в
/// порядке `Permission`).
const BUILTIN_ROLES: [(&str, &str, &str); 2] = [
    (
        "Мастер",
        r#"["orders_edit","clients_view"]"#,
        r#"["orders_edit","stock_view","clients_view"]"#,
    ),
    (
        "Приёмщик",
        r#"["orders_create","orders_edit","clients_view","clients_create","clients_edit"]"#,
        r#"["orders_create","orders_edit","stock_view","clients_view","clients_create","clients_edit"]"#,
    ),
];

/// Права роли как множество; не JSON-массив строк — `None` (такая роль не готовая).
fn permission_set(json: &str) -> Option<BTreeSet<String>> {
    serde_json::from_str(json).ok()
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        for (order, value) in (0_i32..).zip(PART_TYPES.iter()) {
            let mut insert = Query::insert();
            insert
                .into_table(a("dictionary_items"))
                .columns([a("kind"), a("value"), a("sort_order")])
                .values(["part_type".into(), (*value).into(), order.into()])
                .map_err(|e| DbErr::Migration(format!("стартовые данные: {e}")))?;
            m.exec_stmt(insert).await?;
        }

        let db = m.get_connection();
        let mut select = Query::select();
        select
            .columns([a("id"), a("name"), a("permissions")])
            .from(a("roles"))
            .order_by(a("id"), Order::Asc);
        for row in db.query_all(&select).await? {
            let id: i64 = row.try_get_by_index(0)?;
            let name: String = row.try_get_by_index(1)?;
            let permissions: String = row.try_get_by_index(2)?;
            let Some(&(_, _, with_stock)) = BUILTIN_ROLES.iter().find(|(builtin, before, _)| {
                normalize_search(builtin) == normalize_search(&name)
                    && permission_set(before) == permission_set(&permissions)
            }) else {
                continue;
            };
            let mut update = Query::update();
            update
                .table(a("roles"))
                .value(a("permissions"), with_stock)
                .value(a("version"), Expr::col(a("version")).add(1))
                .and_where(Expr::col(a("id")).eq(id));
            db.execute(&update).await?;
        }
        Ok(())
    }
}
