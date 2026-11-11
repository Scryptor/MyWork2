use std::collections::HashMap;
use std::fmt;

use chrono::NaiveDateTime;
use mw_db::entity::user;
use mw_domain::permission::{Permission, PermissionSet};
use mw_domain::search::normalize_search;
use mw_domain::validation::FieldError;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, NotSet, PaginatorTrait,
    QueryFilter, QueryOrder, Set,
};
use serde_json::{Map, json};

use crate::{Actor, Core, CoreError, NewAuditEntry, write_audit};

use super::roles::{all_roles, parse_permissions};
use super::{MIN_PASSWORD_CHARS, PasswordScheme, hash_password};

const ENTITY: &str = "пользователь";

/// Пользователь без хэша пароля.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub id: i64,
    pub login: String,
    pub display_name: String,
    pub role_id: i64,
    pub active: bool,
    pub created_at: NaiveDateTime,
    pub version: i64,
}

impl From<user::Model> for User {
    fn from(m: user::Model) -> Self {
        Self {
            id: m.id,
            login: m.login,
            display_name: m.display_name,
            role_id: m.role_id,
            active: m.active,
            created_at: m.created_at,
            version: m.version,
        }
    }
}

#[derive(Clone)]
pub struct NewUser {
    pub login: String,
    pub display_name: String,
    pub role_id: i64,
    pub password: String,
}

impl fmt::Debug for NewUser {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NewUser")
            .field("login", &self.login)
            .field("display_name", &self.display_name)
            .field("role_id", &self.role_id)
            .field("password", &"***")
            .finish()
    }
}

#[derive(Debug, Clone, Default)]
pub struct UserPatch {
    pub display_name: Option<String>,
    pub role_id: Option<i64>,
    pub active: Option<bool>,
}

pub(super) async fn all_users(conn: &impl ConnectionTrait) -> Result<Vec<user::Model>, CoreError> {
    Ok(user::Entity::find()
        .order_by_asc(user::Column::Id)
        .all(conn)
        .await?)
}

/// Сколько активных пользователей имеют роль с правом `users_manage`.
pub(super) fn admin_count(
    users: &[user::Model],
    role_permissions: &HashMap<i64, PermissionSet>,
) -> usize {
    users
        .iter()
        .filter(|u| {
            u.active
                && role_permissions
                    .get(&u.role_id)
                    .is_some_and(|p| p.contains(Permission::UsersManage))
        })
        .count()
}

fn too_short(password: &str) -> bool {
    password.chars().count() < MIN_PASSWORD_CHARS
}

impl Core {
    /// Истина, пока нет ни одного активного пользователя: вход не требуется.
    pub async fn is_open_mode(&self) -> Result<bool, CoreError> {
        let active = user::Entity::find()
            .filter(user::Column::Active.eq(true))
            .count(&self.db)
            .await?;
        Ok(active == 0)
    }

    pub async fn list_users(&self, actor: &Actor) -> Result<Vec<User>, CoreError> {
        actor.require(Permission::UsersManage)?;
        Ok(all_users(&self.db)
            .await?
            .into_iter()
            .map(User::from)
            .collect())
    }

    pub async fn create_user(&self, actor: &Actor, new: NewUser) -> Result<User, CoreError> {
        actor.require(Permission::UsersManage)?;
        let tx = self.begin_write().await?;
        let login = new.login.trim().to_owned();
        let login_key = normalize_search(&login);
        let mut errors = Vec::new();
        if login.is_empty() {
            errors.push(FieldError::required("login"));
        } else if user::Entity::find()
            .filter(user::Column::LoginKey.eq(login_key.clone()))
            .count(&tx)
            .await?
            > 0
        {
            errors.push(FieldError::new("login", "duplicate"));
        }
        if !all_roles(&tx).await?.iter().any(|r| r.id == new.role_id) {
            errors.push(FieldError::new("role_id", "invalid"));
        }
        if too_short(&new.password) {
            errors.push(FieldError::new("password", "too_short"));
        }
        if !errors.is_empty() {
            return Err(CoreError::Validation(errors));
        }

        let now = self.now();
        let display_name = new.display_name.trim().to_owned();
        let model = user::ActiveModel {
            id: NotSet,
            login: Set(login.clone()),
            login_key: Set(login_key),
            display_name: Set(display_name),
            role_id: Set(new.role_id),
            password_hash: Set(hash_password(&new.password)?),
            password_scheme: Set(PasswordScheme::Argon2.as_str().to_owned()),
            active: Set(true),
            created_at: Set(now),
            updated_at: Set(now),
            version: Set(1),
        }
        .insert(&tx)
        .await?;
        write_audit(
            &tx,
            now,
            actor,
            NewAuditEntry {
                action: "user.created",
                order_id: None,
                client_id: None,
                summary: format!("Создан пользователь «{login}»"),
                details: Some(json!({ "id": model.id, "role_id": model.role_id })),
            },
        )
        .await?;
        tx.commit().await?;
        Ok(model.into())
    }

    pub async fn update_user(
        &self,
        actor: &Actor,
        id: i64,
        expected_version: i64,
        patch: UserPatch,
    ) -> Result<User, CoreError> {
        actor.require(Permission::UsersManage)?;
        let tx = self.begin_write().await?;
        let users = all_users(&tx).await?;
        let current = users
            .iter()
            .find(|u| u.id == id)
            .cloned()
            .ok_or(CoreError::NotFound { entity: ENTITY, id })?;
        if current.version != expected_version {
            return Err(CoreError::Conflict {
                entity: ENTITY,
                id,
                current_version: current.version,
            });
        }
        let roles = all_roles(&tx).await?;
        if let Some(role_id) = patch.role_id
            && !roles.iter().any(|r| r.id == role_id)
        {
            return Err(CoreError::Validation(vec![FieldError::new(
                "role_id", "invalid",
            )]));
        }

        let display_name = patch
            .display_name
            .as_deref()
            .map_or_else(|| current.display_name.clone(), |v| v.trim().to_owned());
        let role_id = patch.role_id.unwrap_or(current.role_id);
        let active = patch.active.unwrap_or(current.active);

        if role_id != current.role_id || active != current.active {
            let role_perms: HashMap<i64, PermissionSet> = roles
                .iter()
                .map(|r| (r.id, parse_permissions(&r.permissions)))
                .collect();
            let before = admin_count(&users, &role_perms);
            let after_users: Vec<user::Model> = users
                .iter()
                .cloned()
                .map(|mut u| {
                    if u.id == id {
                        u.role_id = role_id;
                        u.active = active;
                    }
                    u
                })
                .collect();
            if before > 0 && admin_count(&after_users, &role_perms) == 0 {
                let field = if active != current.active {
                    "active"
                } else {
                    "role_id"
                };
                return Err(CoreError::Validation(vec![FieldError::new(
                    field,
                    "last_admin",
                )]));
            }
        }

        let mut changed = Map::new();
        if display_name != current.display_name {
            changed.insert("display_name".into(), json!(display_name));
        }
        if role_id != current.role_id {
            changed.insert("role_id".into(), json!(role_id));
        }
        if active != current.active {
            changed.insert("active".into(), json!(active));
        }
        let login = current.login.clone();
        let now = self.now();
        let mut model: user::ActiveModel = current.into();
        model.display_name = Set(display_name);
        model.role_id = Set(role_id);
        model.active = Set(active);
        model.updated_at = Set(now);
        model.version = Set(expected_version + 1);
        let model = model.update(&tx).await?;
        write_audit(
            &tx,
            now,
            actor,
            NewAuditEntry {
                action: "user.updated",
                order_id: None,
                client_id: None,
                summary: format!("Изменён пользователь «{login}»"),
                details: Some(json!({ "id": id, "changed": changed })),
            },
        )
        .await?;
        tx.commit().await?;
        Ok(model.into())
    }

    /// Свой пароль можно менять без прав, чужой — только с `users_manage`. Всегда схема `argon2`.
    pub async fn set_password(
        &self,
        actor: &Actor,
        user_id: i64,
        password: &str,
    ) -> Result<(), CoreError> {
        if actor.user_id != Some(user_id) {
            actor.require(Permission::UsersManage)?;
        }
        if too_short(password) {
            return Err(CoreError::Validation(vec![FieldError::new(
                "password",
                "too_short",
            )]));
        }
        let tx = self.begin_write().await?;
        let current =
            user::Entity::find_by_id(user_id)
                .one(&tx)
                .await?
                .ok_or(CoreError::NotFound {
                    entity: ENTITY,
                    id: user_id,
                })?;
        let login = current.login.clone();
        let now = self.now();
        let mut model: user::ActiveModel = current.into();
        model.password_hash = Set(hash_password(password)?);
        model.password_scheme = Set(PasswordScheme::Argon2.as_str().to_owned());
        model.updated_at = Set(now);
        model.update(&tx).await?;
        write_audit(
            &tx,
            now,
            actor,
            NewAuditEntry {
                action: "user.password_changed",
                order_id: None,
                client_id: None,
                summary: format!("Изменён пароль пользователя «{login}»"),
                details: Some(json!({ "id": user_id })),
            },
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_user_debug_hides_password() {
        let new_user = NewUser {
            login: "boss".into(),
            display_name: "Шеф".into(),
            role_id: 1,
            password: "sup3r-secret".into(),
        };
        let text = format!("{new_user:?}");
        assert!(!text.contains("sup3r-secret"));
        assert!(text.contains("***"));
    }
}
