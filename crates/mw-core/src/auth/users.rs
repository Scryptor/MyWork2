use std::collections::HashMap;
use std::fmt;

use chrono::NaiveDateTime;
use mw_db::entity::user::no_login_key;
use mw_db::entity::{session, user};
use mw_domain::permission::{Permission, PermissionSet};
use mw_domain::search::normalize_search;
use mw_domain::validation::FieldError;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, NotSet, PaginatorTrait,
    QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, json};

use crate::{Actor, Core, CoreError, NewAuditEntry, write_audit};

use super::password::{hash_password, verify_password};
use super::roles::{all_roles, parse_permissions};
use super::{MIN_PASSWORD_CHARS, PasswordScheme};

const ENTITY: &str = "пользователь";

/// Пользователь без хэша пароля.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct User {
    /// Номер пользователя.
    pub id: i64,
    /// Логин; пустой, если логина нет (бывает только у пользователя без входа).
    pub login: String,
    /// Отображаемое имя.
    pub display_name: String,
    /// Номер роли.
    pub role_id: i64,
    /// Учётная запись активна.
    pub active: bool,
    /// Может входить в программу. Пользователь без входа не входит ни с каким паролем и не
    /// считается ни в режиме без входа, ни в защите от потери доступа администратора.
    pub can_sign_in: bool,
    /// Отметка «Мастер»: пользователя можно выбрать мастером заказа.
    pub is_master: bool,
    /// Когда создан (местное время предприятия).
    pub created_at: NaiveDateTime,
    /// Версия записи для проверки конфликтов правок.
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
            can_sign_in: m.can_sign_in,
            is_master: m.is_master,
            created_at: m.created_at,
            version: m.version,
        }
    }
}

/// Данные нового пользователя; пароль в логи не попадает.
#[derive(Clone, Deserialize, ts_rs::TS, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewUser {
    /// Логин; обязателен, если пользователь может входить. Без входа — может быть пустым.
    pub login: String,
    /// Отображаемое имя; обязательно, если логин пуст.
    pub display_name: String,
    /// Номер роли.
    pub role_id: i64,
    /// Пароль; обязателен, если пользователь может входить. Без входа — может быть пустым.
    pub password: String,
    /// Может входить в программу.
    pub can_sign_in: bool,
    /// Отметка «Мастер».
    pub is_master: bool,
}

impl fmt::Debug for NewUser {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NewUser")
            .field("login", &self.login)
            .field("display_name", &self.display_name)
            .field("role_id", &self.role_id)
            .field("password", &"***")
            .field("can_sign_in", &self.can_sign_in)
            .field("is_master", &self.is_master)
            .finish()
    }
}

/// Частичное изменение пользователя; отсутствующее поле не меняется; пароль в логи не попадает.
#[derive(Clone, Default, PartialEq, Eq, Deserialize, ts_rs::TS, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
#[ts(optional_fields)]
pub struct UserPatch {
    /// Отображаемое имя.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub display_name: Option<String>,
    /// Номер роли.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub role_id: Option<i64>,
    /// Учётная запись активна.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub active: Option<bool>,
    /// Может входить в программу. Включить вход можно, только если у пользователя есть логин и
    /// пароль — прежние или переданные в этой же правке. Выключение закрывает его сеансы,
    /// логин и пароль сохраняются.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub can_sign_in: Option<bool>,
    /// Отметка «Мастер».
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub is_master: Option<bool>,
    /// Логин; пустая строка — без логина (только у пользователя без входа).
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub login: Option<String>,
    /// Новый пароль. Как при сбросе пароля: схема `argon2`, сеансы пользователя закрываются.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub password: Option<String>,
}

impl fmt::Debug for UserPatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UserPatch")
            .field("display_name", &self.display_name)
            .field("role_id", &self.role_id)
            .field("active", &self.active)
            .field("can_sign_in", &self.can_sign_in)
            .field("is_master", &self.is_master)
            .field("login", &self.login)
            .field("password", &self.password.as_ref().map(|_| "***"))
            .finish()
    }
}

/// Мастер для выбора в заказе: пользователь с отметкой «Мастер».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct MasterUser {
    /// Номер пользователя.
    pub id: i64,
    /// Имя мастера: отображаемое имя, а если оно пустое — логин.
    pub display_name: String,
    /// Учётная запись активна; отключённого мастера не предлагают для новых заказов.
    pub active: bool,
}

/// Имя для людей: отображаемое имя, а если оно пустое — логин.
fn shown_name(u: &user::Model) -> &str {
    if u.display_name.is_empty() {
        &u.login
    } else {
        &u.display_name
    }
}

/// Кто это в журнале: логин, а у пользователя без логина — отображаемое имя.
fn audit_label(login: &str, display_name: &str) -> String {
    if login.is_empty() {
        display_name.to_owned()
    } else {
        login.to_owned()
    }
}

/// Занят ли ключ логина кем-то, кроме `except`.
async fn login_taken(
    conn: &impl ConnectionTrait,
    login_key: &str,
    except: Option<i64>,
) -> Result<bool, CoreError> {
    let mut query = user::Entity::find().filter(user::Column::LoginKey.eq(login_key));
    if let Some(id) = except {
        query = query.filter(user::Column::Id.ne(id));
    }
    Ok(query.count(conn).await? > 0)
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

/// (число пользователей, которые могут войти, — активных и с `can_sign_in`; число таких с правом
/// `users_manage`).
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

/// Пары (может войти, роль) всех пользователей. Войти может активный пользователь с
/// `can_sign_in`; пользователь без входа не вернёт доступ, поэтому в защите не считается.
pub(super) fn accounts(users: &[user::Model]) -> Vec<(bool, i64)> {
    users
        .iter()
        .map(|u| (u.active && u.can_sign_in, u.role_id))
        .collect()
}

/// Истина, если изменение ведёт к блокировке: после него нет ни одного пользователя, который
/// может войти (активен и с `can_sign_in`), с правом `users_manage`, хотя до него либо такой был,
/// либо система была в открытом режиме (никто не может войти), а после кто-то может. Уже
/// нарушенное состояние (входящие пользователи без администратора) посторонние правки не блокирует.
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
    /// Истина, пока нет ни одного активного пользователя, который может входить: вход не
    /// требуется. Пользователи без входа (мастера, которые в программе не работают) не считаются.
    pub async fn is_open_mode(&self) -> Result<bool, CoreError> {
        let can_sign_in = user::Entity::find()
            .filter(user::Column::Active.eq(true))
            .filter(user::Column::CanSignIn.eq(true))
            .count(&self.db)
            .await?;
        Ok(can_sign_in == 0)
    }

    pub async fn list_users(&self, actor: &Actor) -> Result<Vec<User>, CoreError> {
        actor.require(Permission::UsersManage)?;
        Ok(all_users(&self.db)
            .await?
            .into_iter()
            .map(User::from)
            .collect())
    }

    /// Список мастеров для выбора в заказе: все пользователи с отметкой «Мастер», в том числе
    /// отключённые и без входа; активные первыми, затем по имени без учёта регистра.
    /// Доступен любому вошедшему (и в режиме без входа): особого права не нужно.
    pub async fn list_masters(&self, _actor: &Actor) -> Result<Vec<MasterUser>, CoreError> {
        let mut masters: Vec<MasterUser> = user::Entity::find()
            .filter(user::Column::IsMaster.eq(true))
            .all(&self.db)
            .await?
            .iter()
            .map(|u| MasterUser {
                id: u.id,
                display_name: shown_name(u).to_owned(),
                active: u.active,
            })
            .collect();
        masters.sort_by_cached_key(|m| (!m.active, normalize_search(&m.display_name), m.id));
        Ok(masters)
    }

    /// Новый пользователь. Со входом нужны логин и пароль; без входа они не обязательны
    /// (переданные сохраняются), но без логина нужно имя.
    pub async fn create_user(&self, actor: &Actor, new: NewUser) -> Result<User, CoreError> {
        actor.require(Permission::UsersManage)?;
        // Хэш считается до блокировки записи; при ошибке валидации он просто отбрасывается.
        // Без входа пустой пароль не задаётся вовсе.
        let password_hash = if !new.can_sign_in && new.password.is_empty() {
            Ok(None)
        } else {
            self.checked_hash(&new.password).await?.map(Some)
        };
        let tx = self.begin_write().await?;
        let login = new.login.trim().to_owned();
        let display_name = new.display_name.trim().to_owned();
        let mut errors = Vec::new();
        let login_key = if login.is_empty() {
            if new.can_sign_in {
                errors.push(FieldError::required("login"));
            } else if display_name.is_empty() {
                errors.push(FieldError::required("display_name"));
            }
            None
        } else {
            let key = normalize_search(&login);
            if login_taken(&tx, &key, None).await? {
                errors.push(FieldError::new("login", "duplicate"));
            }
            Some(key)
        };
        let roles = all_roles(&tx).await?;
        if !roles.iter().any(|r| r.id == new.role_id) {
            errors.push(FieldError::new("role_id", "invalid"));
        } else {
            let perms = role_permissions(&roles);
            let before = accounts(&all_users(&tx).await?);
            let mut after = before.clone();
            after.push((new.can_sign_in, new.role_id));
            if would_lock_out((&before, &perms), (&after, &perms)) {
                errors.push(FieldError::new("role_id", "last_admin"));
            }
        }
        let password_hash = match password_hash {
            Ok(hash) => hash,
            Err(e) => {
                errors.push(e);
                None
            }
        };
        if !errors.is_empty() {
            return Err(CoreError::Validation(errors));
        }
        let login_key = match login_key {
            Some(key) => key,
            None => no_login_key()?,
        };

        let now = self.now();
        let label = audit_label(&login, &display_name);
        let model = user::ActiveModel {
            id: NotSet,
            login: Set(login),
            login_key: Set(login_key),
            display_name: Set(display_name),
            role_id: Set(new.role_id),
            password_hash: Set(password_hash.unwrap_or_default()),
            password_scheme: Set(PasswordScheme::Argon2.as_str().to_owned()),
            active: Set(true),
            created_at: Set(now),
            updated_at: Set(now),
            version: Set(1),
            can_sign_in: Set(new.can_sign_in),
            is_master: Set(new.is_master),
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
                summary: format!("Создан пользователь «{label}»"),
                details: Some(json!({
                    "id": model.id,
                    "role_id": model.role_id,
                    "can_sign_in": model.can_sign_in,
                    "is_master": model.is_master,
                })),
            },
        )
        .await?;
        tx.commit().await?;
        Ok(model.into())
    }

    /// Хэш Argon2 пароля или ошибка поля `password` (слишком короткий).
    async fn checked_hash(&self, password: &str) -> Result<Result<String, FieldError>, CoreError> {
        if too_short(password) {
            return Ok(Err(FieldError::new("password", "too_short")));
        }
        let plain = password.to_owned();
        Ok(Ok(self
            .hash_gate
            .run(move || hash_password(&plain))
            .await??))
    }

    pub async fn update_user(
        &self,
        actor: &Actor,
        id: i64,
        expected_version: i64,
        patch: UserPatch,
    ) -> Result<User, CoreError> {
        actor.require(Permission::UsersManage)?;
        // Новый пароль хэшируется до блокировки записи: такая правка всегда что-то меняет.
        let new_hash = match &patch.password {
            Some(password) => Some(self.checked_hash(password).await?),
            None => None,
        };
        let tx = self.begin_write().await?;
        let users = all_users(&tx).await?;
        let current = users
            .iter()
            .find(|u| u.id == id)
            .cloned()
            .ok_or(CoreError::NotFound { entity: ENTITY, id })?;
        let roles = all_roles(&tx).await?;
        let display_name = patch
            .display_name
            .as_deref()
            .map_or_else(|| current.display_name.clone(), |v| v.trim().to_owned());
        let role_id = patch.role_id.unwrap_or(current.role_id);
        let active = patch.active.unwrap_or(current.active);
        let can_sign_in = patch.can_sign_in.unwrap_or(current.can_sign_in);
        let is_master = patch.is_master.unwrap_or(current.is_master);
        let login = patch
            .login
            .as_deref()
            .map_or_else(|| current.login.clone(), |v| v.trim().to_owned());

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
        if can_sign_in != current.can_sign_in {
            changed.insert("can_sign_in".into(), json!(can_sign_in));
        }
        if is_master != current.is_master {
            changed.insert("is_master".into(), json!(is_master));
        }
        if login != current.login {
            changed.insert("login".into(), json!(login));
        }
        if new_hash.is_some() {
            // Сам пароль в журнал не попадает.
            changed.insert("password".into(), json!(true));
        }
        if changed.is_empty() {
            return Ok(current.into());
        }
        if current.version != expected_version {
            return Err(CoreError::Conflict {
                entity: ENTITY,
                id,
                current_version: current.version,
            });
        }
        if !roles.iter().any(|r| r.id == role_id) {
            return Err(CoreError::Validation(vec![FieldError::new(
                "role_id", "invalid",
            )]));
        }

        let mut errors = Vec::new();
        // None — нужен новый ключ пользователя без логина.
        let login_key = if login == current.login {
            Some(current.login_key.clone())
        } else if login.is_empty() {
            None
        } else {
            let key = normalize_search(&login);
            if login_taken(&tx, &key, Some(id)).await? {
                errors.push(FieldError::new("login", "duplicate"));
            }
            Some(key)
        };
        // Логин и пароль проверяются, только когда вход включают (или убирают логин): прежних
        // пользователей без хэша (импорт MyWork 2 с непереносимым паролем) можно править и дальше.
        let turning_on = can_sign_in && !current.can_sign_in;
        if login.is_empty() {
            if can_sign_in && (turning_on || login != current.login) {
                errors.push(FieldError::required("login"));
            } else if !can_sign_in && display_name.is_empty() {
                errors.push(FieldError::required("display_name"));
            }
        }
        let password_hash = match new_hash {
            Some(Ok(hash)) => Some(hash),
            Some(Err(e)) => {
                errors.push(e);
                None
            }
            None => {
                if turning_on && current.password_hash.is_empty() {
                    errors.push(FieldError::required("password"));
                }
                None
            }
        };
        if !errors.is_empty() {
            return Err(CoreError::Validation(errors));
        }

        if role_id != current.role_id
            || active != current.active
            || can_sign_in != current.can_sign_in
        {
            let perms = role_permissions(&roles);
            let before = accounts(&users);
            let after: Vec<(bool, i64)> = users
                .iter()
                .map(|u| {
                    if u.id == id {
                        (active && can_sign_in, role_id)
                    } else {
                        (u.active && u.can_sign_in, u.role_id)
                    }
                })
                .collect();
            if would_lock_out((&before, &perms), (&after, &perms)) {
                let field = if can_sign_in != current.can_sign_in {
                    "can_sign_in"
                } else if active != current.active {
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
        let login_key = match login_key {
            Some(key) => key,
            None => no_login_key()?,
        };

        let label = audit_label(&current.login, &current.display_name);
        let could_sign_in = current.active && current.can_sign_in;
        let password_changed = password_hash.is_some();
        let now = self.now();
        let mut model: user::ActiveModel = current.into();
        model.display_name = Set(display_name);
        model.role_id = Set(role_id);
        model.active = Set(active);
        model.can_sign_in = Set(can_sign_in);
        model.is_master = Set(is_master);
        model.login = Set(login);
        model.login_key = Set(login_key);
        if let Some(hash) = password_hash {
            model.password_hash = Set(hash);
            model.password_scheme = Set(PasswordScheme::Argon2.as_str().to_owned());
        }
        model.updated_at = Set(now);
        model.version = Set(expected_version + 1);
        let model = model.update(&tx).await?;
        if (could_sign_in && !(active && can_sign_in)) || password_changed {
            // Отключённого пользователя и пользователя, которому выключили вход, выкидываем из
            // системы; после повторного включения нужен новый вход. Новый пароль тоже закрывает
            // сеансы, как сброс пароля.
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
                summary: format!("Изменён пользователь «{label}»"),
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
        let plain = password.to_owned();
        let password_hash = self.hash_gate.run(move || hash_password(&plain)).await??;
        let tx = self.begin_write().await?;
        let current =
            user::Entity::find_by_id(user_id)
                .one(&tx)
                .await?
                .ok_or(CoreError::NotFound {
                    entity: ENTITY,
                    id: user_id,
                })?;
        let label = audit_label(&current.login, &current.display_name);
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
                summary: format!("Изменён пароль пользователя «{label}»"),
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
            .filter(|u| u.active && u.can_sign_in)
            .ok_or(CoreError::Unauthenticated)?;
        let invalid_current =
            || CoreError::Validation(vec![FieldError::new("current_password", "invalid")]);
        // Проверка и хэширование — вне блокировки записи.
        let verified = match PasswordScheme::from_code(&stored.password_scheme) {
            Some(scheme) => {
                let (hash, plain) = (stored.password_hash.clone(), current_password.to_owned());
                self.hash_gate
                    .run(move || verify_password(scheme, &hash, &plain))
                    .await?
            }
            None => false,
        };
        if !verified {
            return Err(invalid_current());
        }
        if too_short(new_password) {
            return Err(CoreError::Validation(vec![FieldError::new(
                "new_password",
                "too_short",
            )]));
        }
        let plain = new_password.to_owned();
        let new_hash = self.hash_gate.run(move || hash_password(&plain)).await??;
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
            .filter(user::Column::CanSignIn.eq(true))
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
            can_sign_in: true,
            is_master: false,
        };
        let text = format!("{new_user:?}");
        assert!(!text.contains("sup3r-secret"));
        assert!(text.contains("***"));
    }

    #[test]
    fn user_patch_debug_hides_password() {
        let patch = UserPatch {
            login: Some("boss".into()),
            password: Some("sup3r-secret".into()),
            ..Default::default()
        };
        let text = format!("{patch:?}");
        assert!(!text.contains("sup3r-secret"), "{text}");
        assert!(text.contains("***") && text.contains("boss"), "{text}");
        assert!(format!("{:?}", UserPatch::default()).contains("password: None"));
    }
}
