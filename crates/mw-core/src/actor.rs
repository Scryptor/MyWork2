use mw_domain::permission::{Permission, PermissionSet};

use crate::CoreError;

/// Кто выполняет операцию: пользователь с правами или внутренний вызов без ограничений.
#[derive(Debug, Clone)]
pub struct Actor {
    pub(crate) user_id: Option<i64>,
    pub(crate) name: String,
    pub(crate) permissions: PermissionSet,
}

impl Actor {
    /// Все права, без привязки к пользователю (первоначальная настройка, тесты, служебные вызовы).
    pub fn unrestricted(name: impl Into<String>) -> Actor {
        Actor {
            user_id: None,
            name: name.into(),
            permissions: PermissionSet::all(),
        }
    }

    /// Только для тестов: `Actor` с произвольными правами.
    #[cfg(any(test, feature = "testing"))]
    pub fn for_tests(
        user_id: Option<i64>,
        name: impl Into<String>,
        permissions: PermissionSet,
    ) -> Actor {
        Actor {
            user_id,
            name: name.into(),
            permissions,
        }
    }

    pub fn user_id(&self) -> Option<i64> {
        self.user_id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn permissions(&self) -> &PermissionSet {
        &self.permissions
    }

    pub fn require(&self, permission: Permission) -> Result<(), CoreError> {
        if self.permissions.contains(permission) {
            Ok(())
        } else {
            Err(CoreError::Forbidden(permission))
        }
    }

    /// Достаточно любого из прав; при отказе сообщается первое из списка.
    pub fn require_any(&self, permissions: &[Permission]) -> Result<(), CoreError> {
        if permissions.iter().any(|p| self.permissions.contains(*p)) {
            return Ok(());
        }
        match permissions.first() {
            Some(first) => Err(CoreError::Forbidden(*first)),
            None => Ok(()),
        }
    }
}
