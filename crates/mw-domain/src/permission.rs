use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

code_enum! {
    /// Право доступа. Порядок объявления определяет порядок в наборе и в JSON.
    Permission {
        OrdersCreate => "orders_create",
        OrdersEdit => "orders_edit",
        OrdersDelete => "orders_delete",
        OrderDatesEdit => "order_dates_edit",
        ReportsView => "reports_view",
        StockView => "stock_view",
        StockCreate => "stock_create",
        StockEdit => "stock_edit",
        StockDelete => "stock_delete",
        ClientsView => "clients_view",
        ClientsCreate => "clients_create",
        ClientsEdit => "clients_edit",
        ClientsDelete => "clients_delete",
        ClientsMerge => "clients_merge",
        SettingsManage => "settings_manage",
        UsersManage => "users_manage",
        AuditView => "audit_view",
    }
}

impl Permission {
    pub const ALL: [Permission; 17] = [
        Self::OrdersCreate,
        Self::OrdersEdit,
        Self::OrdersDelete,
        Self::OrderDatesEdit,
        Self::ReportsView,
        Self::StockView,
        Self::StockCreate,
        Self::StockEdit,
        Self::StockDelete,
        Self::ClientsView,
        Self::ClientsCreate,
        Self::ClientsEdit,
        Self::ClientsDelete,
        Self::ClientsMerge,
        Self::SettingsManage,
        Self::UsersManage,
        Self::AuditView,
    ];

    pub fn label_ru(self) -> &'static str {
        match self {
            Self::OrdersCreate => "Добавление заказов",
            Self::OrdersEdit => "Изменение заказов",
            Self::OrdersDelete => "Удаление заказов",
            Self::OrderDatesEdit => "Изменение дат заказов",
            Self::ReportsView => "Отчёты",
            Self::StockView => "Просмотр склада",
            Self::StockCreate => "Добавление на склад",
            Self::StockEdit => "Изменение склада",
            Self::StockDelete => "Удаление со склада",
            Self::ClientsView => "Просмотр клиентов",
            Self::ClientsCreate => "Добавление клиентов",
            Self::ClientsEdit => "Изменение клиентов",
            Self::ClientsDelete => "Удаление клиентов",
            Self::ClientsMerge => "Объединение клиентов",
            Self::SettingsManage => "Настройки",
            Self::UsersManage => "Пользователи и права",
            Self::AuditView => "Журнал действий",
        }
    }
}

/// Упорядоченное (по порядку объявления) множество прав.
#[derive(Debug, Clone, Default, PartialEq, Eq, ts_rs::TS, utoipa::ToSchema)]
pub struct PermissionSet(BTreeSet<Permission>);

impl PermissionSet {
    pub fn all() -> Self {
        Self(Permission::ALL.into_iter().collect())
    }

    pub fn empty() -> Self {
        Self::default()
    }

    pub fn contains(&self, permission: Permission) -> bool {
        self.0.contains(&permission)
    }

    pub fn insert(&mut self, permission: Permission) -> bool {
        self.0.insert(permission)
    }

    pub fn iter(&self) -> impl Iterator<Item = Permission> + '_ {
        self.0.iter().copied()
    }

    /// Неизвестные коды пропускаются.
    pub fn from_codes<'a>(codes: impl IntoIterator<Item = &'a str>) -> Self {
        Self(codes.into_iter().filter_map(|c| c.parse().ok()).collect())
    }
}

impl Serialize for PermissionSet {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.0.iter().map(|p| p.as_str()))
    }
}

impl<'de> Deserialize<'de> for PermissionSet {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let codes = Vec::<String>::deserialize(deserializer)?;
        Ok(Self::from_codes(codes.iter().map(String::as_str)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_codes_roundtrip_and_unknown_codes_are_ignored() {
        for p in Permission::ALL {
            assert_eq!(p.as_str().parse::<Permission>(), Ok(p));
        }
        let set: PermissionSet =
            serde_json::from_str(r#"["users_manage","future_perm","orders_create"]"#).unwrap();
        assert_eq!(
            serde_json::to_string(&set).unwrap(),
            r#"["orders_create","users_manage"]"#
        );
        assert_eq!(PermissionSet::all().iter().count(), 17);
    }
}
