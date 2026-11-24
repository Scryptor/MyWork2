//! Готовые роли «Мастер» и «Приёмщик» — один список для всего, что пишет роли не миграцией: импорт
//! MyWork 2 стирает роли базы и после ролей пакета добавляет отсюда те, которых нет.
//!
//! Миграции готовых ролей (`m20261120_000002_builtin_roles`, `m20261124_000002_stock_data`) держат
//! свои замороженные литералы: смена набора или прав — новая миграция данных и правка этого списка.
//! Права здесь — в порядке `Permission`, такие же, как у ролей новой базы.

use mw_domain::search::normalize_search;
use sea_orm::sea_query::{Alias, Query};
use sea_orm::{ConnectionTrait, DbErr};

use crate::masters::{MASTER_ROLE, MASTER_ROLE_PERMISSIONS};

/// Готовая роль приёмщика.
pub const RECEPTIONIST_ROLE: &str = "Приёмщик";
/// Права роли «Приёмщик» в JSON, в порядке `Permission`.
pub const RECEPTIONIST_ROLE_PERMISSIONS: &str = r#"["orders_create","orders_edit","stock_view","clients_view","clients_create","clients_edit"]"#;

/// (название, права в JSON) готовых ролей. «Администратор» (все права) сюда не входит: он есть в
/// пакете импорта и в начальной настройке.
pub const BUILTIN_ROLES: [(&str, &str); 2] = [
    (MASTER_ROLE, MASTER_ROLE_PERMISSIONS),
    (RECEPTIONIST_ROLE, RECEPTIONIST_ROLE_PERMISSIONS),
];

/// Добавляет готовые роли, которых нет: роль с тем же названием без учёта регистра и «ё»
/// (`normalize_search`) — та же роль, её название и права не меняются. Возвращает названия
/// добавленных в порядке списка.
pub async fn ensure_builtin_roles(conn: &impl ConnectionTrait) -> Result<Vec<&'static str>, DbErr> {
    let mut select = Query::select();
    select.column(Alias::new("name")).from(Alias::new("roles"));
    let mut taken = Vec::new();
    for row in conn.query_all(&select).await? {
        let name: String = row.try_get_by_index(0)?;
        taken.push(normalize_search(&name));
    }
    let mut added = Vec::new();
    for (name, permissions) in BUILTIN_ROLES {
        if taken.contains(&normalize_search(name)) {
            continue;
        }
        let mut insert = Query::insert();
        insert
            .into_table(Alias::new("roles"))
            .columns([
                Alias::new("name"),
                Alias::new("permissions"),
                Alias::new("version"),
            ])
            .values([name.into(), permissions.into(), 1_i64.into()])
            .map_err(|e| DbErr::Custom(format!("готовая роль «{name}»: {e}")))?;
        conn.execute(&insert).await?;
        added.push(name);
    }
    Ok(added)
}

#[cfg(test)]
mod tests {
    use mw_domain::permission::{Permission, PermissionSet};

    use super::*;

    /// Права готовых ролей — в порядке `Permission` (как их записывает правка роли) и с просмотром
    /// склада: мастер списывает запчасти, приёмщик отвечает, есть ли запчасть на складе.
    #[test]
    fn builtin_roles_list_permissions_in_order_with_stock_view() {
        for (name, permissions) in BUILTIN_ROLES {
            let parsed: PermissionSet = serde_json::from_str(permissions).unwrap();
            assert_eq!(
                serde_json::to_string(&parsed).unwrap(),
                permissions,
                "{name}"
            );
            assert!(parsed.contains(Permission::StockView), "{name}");
        }
    }
}
