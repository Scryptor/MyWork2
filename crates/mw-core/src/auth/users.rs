use std::collections::HashMap;
use std::fmt;

use chrono::NaiveDateTime;
use mw_db::entity::{session, user};
use mw_domain::permission::{Permission, PermissionSet};
use mw_domain::search::normalize_search;
use mw_domain::validation::FieldError;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, NotSet, PaginatorTrait,
    QueryFilter, QueryOrder, Set,
};
use serde_json::{Map, json};

use crate::{Actor, Core, CoreError, NewAuditEntry, write_audit};

use super::password::{hash_password_blocking, verify_password_blocking};
use super::roles::{all_roles, parse_permissions};
use super::{MIN_PASSWORD_CHARS, PasswordScheme};

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

/// Права ролей по id.
pub(super) fn role_permissions(
    roles: &[mw_db::entity::role::Model],
) -> HashMap<i64, PermissionSet> {
    roles
        .iter()
        .map(|r| (r.id, parse_permissions(&r.permissions)))
        .collect()
}

/// (число активных пользователей, число активных пользователей с правом `users_manage`).
fn access_counts(
    accounts: &[(bool, i64)],
    role_permissions: &HashMap<i64, PermissionSet>,
) -> (usize, usize) {
    let active = accounts.iter().filter(|(active, _)| *active).count();
    let admins = accounts
        .iter()
        .filter(|&&(active, role_id)| {
            active
                && role_permissions
                    .get(&role_id)
                    .is_some_and(|p| p.contains(Permission::UsersManage))
        })
        .count();
    (active, admins)
}

/// Пары (активен, роль) всех пользователей.
pub(super) fn accounts(users: &[user::Model]) -> Vec<(bool, i64)> {
    users.iter().map(|u| (u.active, u.role_id)).collect()
}

/// Истина, если изменение ведёт к блокировке: после него нет ни одного активного пользователя
/// с правом `users_manage`, хотя до него либо такой был, либо система была в открытом режиме
/// (нет активных пользователей), а после появились активные. Уже нарушенное состояние
/// (активные пользователи без администратора) посторонние правки не блокирует.
/// Единственная проверка для `create_user`, `update_user` и `update_role`.
pub(super) fn would_lock_out(
    before: (&[(bool, i64)], &HashMap<i64, PermissionSet>),
    after: (&[(bool, i64)], &HashMap<i64, PermissionSet>),
) -> bool {
    let (before_active, before_admins) = access_counts(before.0, before.1);
    let (after_active, after_admins) = access_counts(after.0, after.1);
    after_admins == 0 && (before_admins > 0 || (before_active == 0 && after_active > 0))
}

async fn delete_user_sessions(conn: &impl ConnectionTrait, user_id: i64) -> Result<(), CoreError> {
    session::Entity::delete_many()
        .filter(session::Column::UserId.eq(user_id))
        .exec(conn)
        .await?;
    Ok(())
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
        // Хэш считается до блокировки записи; при ошибке валидации он просто отбрасывается.
        let password_hash = if too_short(&new.password) {
            None
        } else {
            Some(hash_password_blocking(new.password.clone()).await?)
        };
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
        let roles = all_roles(&tx).await?;
        if !roles.iter().any(|r| r.id == new.role_id) {
            errors.push(FieldError::new("role_id", "invalid"));
        } else {
            let perms = role_permissions(&roles);
            let before = accounts(&all_users(&tx).await?);
            let mut after = before.clone();
            after.push((true, new.role_id));
            if would_lock_out((&before, &perms), (&after, &perms)) {
                errors.push(FieldError::new("role_id", "last_admin"));
            }
        }
        if password_hash.is_none() {
            errors.push(FieldError::new("password", "too_short"));
        }
        let Some(password_hash) = password_hash.filter(|_| errors.is_empty()) else {
            return Err(CoreError::Validation(errors));
        };

        let now = self.now();
        let display_name = new.display_name.trim().to_owned();
        let model = user::ActiveModel {
            id: NotSet,
            login: Set(login.clone()),
            login_key: Set(login_key),
            display_name: Set(display_name),
            role_id: Set(new.role_id),
            password_hash: Set(password_hash),
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
            let perms = role_permissions(&roles);
            let before = accounts(&users);
            let after: Vec<(bool, i64)> = users
                .iter()
                .map(|u| {
                    if u.id == id {
                        (active, role_id)
                    } else {
                        (u.active, u.role_id)
                    }
                })
                .collect();
            if would_lock_out((&before, &perms), (&after, &perms)) {
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
        if patch.active == Some(false) {
            // Отключённого пользователя выкидываем из системы; после повторного включения нужен новый вход.
            delete_user_sessions(&tx, id).await?;
        }
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

    /// Сброс пароля администратором: нужно `users_manage` для ЛЮБОГО пользователя, включая себя.
    /// Свой пароль меняют через `change_own_password`. Всегда схема `argon2`.
    ///
    /// Все сессии пользователя удаляются в той же транзакции.
    pub async fn set_password(
        &self,
        actor: &Actor,
        user_id: i64,
        password: &str,
    ) -> Result<(), CoreError> {
        actor.require(Permission::UsersManage)?;
        if too_short(password) {
            return Err(CoreError::Validation(vec![FieldError::new(
                "password",
                "too_short",
            )]));
        }
        let password_hash = hash_password_blocking(password.to_owned()).await?;
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
        model.password_hash = Set(password_hash);
        model.password_scheme = Set(PasswordScheme::Argon2.as_str().to_owned());
        model.updated_at = Set(now);
        model.update(&tx).await?;
        delete_user_sessions(&tx, user_id).await?;
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

    /// Смена собственного пароля с проверкой текущего. Все сессии пользователя удаляются
    /// (`Core` не знает текущую сессию вызывающего): после смены нужен новый вход.
    pub async fn change_own_password(
        &self,
        actor: &Actor,
        current_password: &str,
        new_password: &str,
    ) -> Result<(), CoreError> {
        let user_id = actor.user_id.ok_or(CoreError::Unauthenticated)?;
        let stored = user::Entity::find_by_id(user_id)
            .one(&self.db)
            .await?
            .filter(|u| u.active)
            .ok_or(CoreError::Unauthenticated)?;
        let invalid_current =
            || CoreError::Validation(vec![FieldError::new("current_password", "invalid")]);
        // Проверка и хэширование — вне блокировки записи.
        let verified = match PasswordScheme::from_code(&stored.password_scheme) {
            Some(scheme) => {
                verify_password_blocking(
                    scheme,
                    stored.password_hash.clone(),
                    current_password.to_owned(),
                )
                .await
            }
            None => false,
        };
        if !verified {
            return Err(invalid_current());
        }
        if too_short(new_password) {
            return Err(CoreError::Validation(vec![FieldError::new(
                "password",
                "too_short",
            )]));
        }
        let new_hash = hash_password_blocking(new_password.to_owned()).await?;
        let tx = self.begin_write().await?;
        let now = self.now();
        let res = user::Entity::update_many()
            .col_expr(user::Column::PasswordHash, Expr::value(new_hash))
            .col_expr(
                user::Column::PasswordScheme,
                Expr::value(PasswordScheme::Argon2.as_str()),
            )
            .col_expr(user::Column::UpdatedAt, Expr::value(now))
            .filter(user::Column::Id.eq(user_id))
            .filter(user::Column::PasswordHash.eq(stored.password_hash.clone()))
            .filter(user::Column::Active.eq(true))
            .exec(&tx)
            .await?;
        if res.rows_affected != 1 {
            tx.rollback().await?;
            return Err(invalid_current());
        }
        delete_user_sessions(&tx, user_id).await?;
        write_audit(
            &tx,
            now,
            actor,
            NewAuditEntry {
                action: "user.password_changed",
                order_id: None,
                client_id: None,
                summary: format!("Изменён пароль пользователя «{}»", stored.login),
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
