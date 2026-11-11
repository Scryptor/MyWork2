//! Вход по логину и паролю, сессии с непрозрачным токеном и сроком по неактивности.

use std::fmt;
use std::sync::OnceLock;

use chrono::Duration;
use mw_db::entity::{role, session, user};
use mw_domain::permission::PermissionSet;
use mw_domain::search::normalize_search;
use sea_orm::{ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, NotSet, QueryFilter, Set};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::{Actor, Core, CoreError, NewAuditEntry, write_audit};

use super::password::{hash_password, hash_password_blocking, verify_password_blocking};
use super::roles::parse_permissions;
use super::{PasswordScheme, User};

/// Сессия без обращений живёт столько дней.
pub const SESSION_IDLE_DAYS: i64 = 30;
/// `last_seen_at` обновляется не чаще, чем раз в столько секунд.
pub const LAST_SEEN_THROTTLE_SECS: i64 = 60;

/// Результат входа. Открытый токен возвращается один раз; в БД лежит только его sha256.
#[derive(Clone)]
pub struct Session {
    pub token: String,
    pub user: User,
    pub permissions: PermissionSet,
}

impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session")
            .field("token", &"***")
            .field("user", &self.user)
            .field("permissions", &self.permissions)
            .finish()
    }
}

fn internal(msg: &str) -> CoreError {
    CoreError::Db(DbErr::Custom(msg.to_owned()))
}

/// 32 случайных байта из ОС в hex (64 символа).
fn new_token() -> Result<String, CoreError> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| internal("не удалось получить случайные байты"))?;
    Ok(hex::encode(bytes))
}

fn token_hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// Проверка пароля против фиксированного хэша: неизвестный логин отвечает за то же время.
async fn verify_against_dummy(password: &str) {
    static DUMMY: OnceLock<String> = OnceLock::new();
    let password = password.to_owned();
    let _ = tokio::task::spawn_blocking(move || {
        let stored = DUMMY.get_or_init(|| hash_password("пустышка").unwrap_or_default());
        super::verify_password(PasswordScheme::Argon2, stored, &password)
    })
    .await;
}

impl Core {
    pub async fn login(
        &self,
        login: &str,
        password: &str,
        device_label: &str,
    ) -> Result<Session, CoreError> {
        let login = login.trim();
        let found = user::Entity::find()
            .filter(user::Column::LoginKey.eq(normalize_search(login)))
            .one(&self.db)
            .await?;

        let verified = match found {
            Some(u) => {
                let scheme = PasswordScheme::from_code(&u.password_scheme);
                let ok = match scheme {
                    Some(scheme) => {
                        verify_password_blocking(
                            scheme,
                            u.password_hash.clone(),
                            password.to_owned(),
                        )
                        .await
                    }
                    None => false,
                };
                (ok && u.active).then_some((u, scheme))
            }
            None => {
                verify_against_dummy(password).await;
                None
            }
        };
        let Some((u, scheme)) = verified else {
            self.record_failed_login(login).await?;
            return Err(CoreError::InvalidCredentials);
        };

        let upgraded = if scheme == Some(PasswordScheme::LegacySha1) {
            Some(hash_password_blocking(password.to_owned()).await?)
        } else {
            None
        };
        let token = new_token()?;
        let tx = self.begin_write().await?;
        let now = self.now();
        let model = match upgraded {
            Some(hash) => {
                user::ActiveModel {
                    id: Set(u.id),
                    password_hash: Set(hash),
                    password_scheme: Set(PasswordScheme::Argon2.as_str().to_owned()),
                    ..Default::default()
                }
                .update(&tx)
                .await?
            }
            None => u,
        };
        session::ActiveModel {
            id: NotSet,
            user_id: Set(model.id),
            token_hash: Set(token_hash(&token)),
            device_label: Set(device_label.trim().to_owned()),
            created_at: Set(now),
            last_seen_at: Set(now),
        }
        .insert(&tx)
        .await?;
        let permissions = match role::Entity::find_by_id(model.role_id).one(&tx).await? {
            Some(r) => parse_permissions(&r.permissions),
            None => PermissionSet::empty(),
        };
        let actor = actor_for(&model, permissions.clone());
        write_audit(
            &tx,
            now,
            &actor,
            NewAuditEntry {
                action: "user.login",
                order_id: None,
                client_id: None,
                summary: format!("Вход: {}", model.login),
                details: Some(json!({ "device": device_label.trim() })),
            },
        )
        .await?;
        tx.commit().await?;
        Ok(Session {
            token,
            user: model.into(),
            permissions,
        })
    }

    async fn record_failed_login(&self, login: &str) -> Result<(), CoreError> {
        let tx = self.begin_write().await?;
        let actor = Actor {
            user_id: None,
            name: login.to_owned(),
            permissions: PermissionSet::empty(),
        };
        write_audit(
            &tx,
            self.now(),
            &actor,
            NewAuditEntry {
                action: "user.login_failed",
                order_id: None,
                client_id: None,
                summary: format!("Неудачный вход: {login}"),
                details: None,
            },
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Права берутся из роли на момент вызова. Устаревшая сессия удаляется.
    pub async fn authenticate(&self, token: &str) -> Result<Actor, CoreError> {
        let hash = token_hash(token);
        let Some(s) = session::Entity::find()
            .filter(session::Column::TokenHash.eq(hash))
            .one(&self.db)
            .await?
        else {
            return Err(CoreError::Unauthenticated);
        };
        let now = self.now();
        if s.last_seen_at < now - Duration::days(SESSION_IDLE_DAYS) {
            self.delete_session(s.id).await?;
            return Err(CoreError::Unauthenticated);
        }
        let u = user::Entity::find_by_id(s.user_id)
            .one(&self.db)
            .await?
            .filter(|u| u.active)
            .ok_or(CoreError::Unauthenticated)?;
        let permissions = match role::Entity::find_by_id(u.role_id).one(&self.db).await? {
            Some(r) => parse_permissions(&r.permissions),
            None => PermissionSet::empty(),
        };
        if now - s.last_seen_at > Duration::seconds(LAST_SEEN_THROTTLE_SECS) {
            let tx = self.begin_write().await?;
            session::ActiveModel {
                id: Set(s.id),
                last_seen_at: Set(now),
                ..Default::default()
            }
            .update(&tx)
            .await
            .map(|_| ())
            .or_else(|e| match e {
                // сессию удалили (выход) между чтением и записью
                DbErr::RecordNotUpdated => Ok(()),
                other => Err(other),
            })?;
            tx.commit().await?;
        }
        Ok(actor_for(&u, permissions))
    }

    /// Удаляет сессию; если её нет — тоже успех.
    pub async fn logout(&self, token: &str) -> Result<(), CoreError> {
        let tx = self.begin_write().await?;
        session::Entity::delete_many()
            .filter(session::Column::TokenHash.eq(token_hash(token)))
            .exec(&tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn delete_session(&self, id: i64) -> Result<(), CoreError> {
        let tx = self.begin_write().await?;
        session::Entity::delete_by_id(id).exec(&tx).await?;
        tx.commit().await?;
        Ok(())
    }
}

fn actor_for(u: &user::Model, permissions: PermissionSet) -> Actor {
    let name = if u.display_name.is_empty() {
        u.login.clone()
    } else {
        u.display_name.clone()
    };
    Actor {
        user_id: Some(u.id),
        name,
        permissions,
    }
}

#[cfg(test)]
mod tests {
    use mw_domain::permission::PermissionSet;

    use super::*;

    #[test]
    fn session_debug_hides_token() {
        let s = Session {
            token: "секретный-токен-abc123".into(),
            user: User {
                id: 1,
                login: "ирина".into(),
                display_name: String::new(),
                role_id: 1,
                active: true,
                created_at: Default::default(),
                version: 1,
            },
            permissions: PermissionSet::empty(),
        };
        let text = format!("{s:?}");
        assert!(text.contains("\"***\"") && !text.contains("abc123"));
    }

    #[test]
    fn token_is_64_hex_and_hash_is_sha256() {
        let t = new_token().unwrap();
        assert_eq!(t.len(), 64);
        assert!(
            t.bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        );
        assert_eq!(
            token_hash("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
