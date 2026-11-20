//! Вход по логину и паролю, сессии с непрозрачным токеном и сроком по неактивности.

use std::fmt;
use std::sync::OnceLock;

use chrono::Duration;
use mw_db::entity::{role, session, user};
use mw_domain::permission::PermissionSet;
use mw_domain::search::normalize_search;
use sea_orm::sea_query::Expr;
use sea_orm::{ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, NotSet, QueryFilter, Set};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::{Actor, Core, CoreError, NewAuditEntry, write_audit};

use super::password::{hash_password, verify_password};
use super::roles::parse_permissions;
use super::users::{signs_in_condition, user_signs_in};
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
    CoreError::Internal(msg.to_owned())
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

/// Нужна ли «холостая» проверка Argon2 при отказе: когда настоящая Argon2-проверка не выполнялась
/// (неизвестный логин, старая схема SHA-1 или неизвестная схема).
fn needs_dummy_verify(scheme: Option<PasswordScheme>) -> bool {
    scheme != Some(PasswordScheme::Argon2)
}

impl Core {
    /// Проверка пароля против фиксированного хэша: неизвестный логин отвечает за то же время.
    /// Хэш-пустышка тоже считается внутри ограничителя.
    async fn verify_against_dummy(&self, password: &str) -> Result<(), CoreError> {
        static DUMMY: OnceLock<String> = OnceLock::new();
        let password = password.to_owned();
        self.hash_gate
            .run(move || {
                let stored = DUMMY.get_or_init(|| hash_password("пустышка").unwrap_or_default());
                verify_password(PasswordScheme::Argon2, stored, &password);
            })
            .await
    }

    pub async fn login(
        &self,
        login: &str,
        password: &str,
        device_label: &str,
    ) -> Result<Session, CoreError> {
        let login = login.trim();
        // Кто не может войти (отключён, без входа, без пароля), отвечает так же, как неизвестный
        // логин: тот же отказ за то же время.
        let found = user::Entity::find()
            .filter(user::Column::LoginKey.eq(normalize_search(login)))
            .one(&self.db)
            .await?
            .filter(user_signs_in);

        // Схема учётной записи; у неизвестного логина её нет.
        let scheme = found
            .as_ref()
            .and_then(|u| PasswordScheme::from_code(&u.password_scheme));
        let verified = match found {
            Some(u) => {
                let ok = match scheme {
                    Some(scheme) => {
                        let (hash, plain) = (u.password_hash.clone(), password.to_owned());
                        self.hash_gate
                            .run(move || verify_password(scheme, &hash, &plain))
                            .await?
                    }
                    None => false,
                };
                ok.then_some(u)
            }
            None => None,
        };
        let Some(u) = verified else {
            // Любой отказ стоит примерно одного Argon2, чтобы не выдавать тип учётной записи.
            if needs_dummy_verify(scheme) {
                self.verify_against_dummy(password).await?;
            }
            self.record_failed_login(login).await?;
            return Err(CoreError::InvalidCredentials);
        };

        let upgraded = if scheme == Some(PasswordScheme::LegacySha1) {
            {
                let plain = password.to_owned();
                Some(self.hash_gate.run(move || hash_password(&plain)).await??)
            }
        } else {
            None
        };
        match self.commit_login(u, upgraded, device_label).await? {
            Some(session) => Ok(session),
            None => {
                self.record_failed_login(login).await?;
                Err(CoreError::InvalidCredentials)
            }
        }
    }

    /// Создаёт сессию в одной транзакции, заново проверив пользователя: пароль не менялся
    /// (`u.password_hash` тот же, что проверен), он всё ещё активен и может входить.
    /// `None` — проверка не прошла.
    /// `upgraded` — новый хэш Argon2 для пользователя со старой схемой.
    pub(crate) async fn commit_login(
        &self,
        u: user::Model,
        upgraded: Option<String>,
        device_label: &str,
    ) -> Result<Option<Session>, CoreError> {
        let token = new_token()?;
        let tx = self.begin_write().await?;
        let now = self.now();
        let still_valid = match upgraded {
            Some(hash) => {
                let res = user::Entity::update_many()
                    .col_expr(user::Column::PasswordHash, Expr::value(hash))
                    .col_expr(
                        user::Column::PasswordScheme,
                        Expr::value(PasswordScheme::Argon2.as_str()),
                    )
                    .filter(user::Column::Id.eq(u.id))
                    .filter(user::Column::PasswordHash.eq(u.password_hash.clone()))
                    .filter(signs_in_condition())
                    .exec(&tx)
                    .await?;
                res.rows_affected == 1
            }
            None => user::Entity::find_by_id(u.id)
                .one(&tx)
                .await?
                .is_some_and(|cur| user_signs_in(&cur) && cur.password_hash == u.password_hash),
        };
        if !still_valid {
            tx.rollback().await?;
            return Ok(None);
        }
        session::ActiveModel {
            id: NotSet,
            user_id: Set(u.id),
            token_hash: Set(token_hash(&token)),
            device_label: Set(device_label.trim().to_owned()),
            created_at: Set(now),
            last_seen_at: Set(now),
        }
        .insert(&tx)
        .await?;
        let permissions = match role::Entity::find_by_id(u.role_id).one(&tx).await? {
            Some(r) => parse_permissions(&r.permissions),
            None => PermissionSet::empty(),
        };
        let actor = actor_for(&u, permissions.clone());
        write_audit(
            &tx,
            now,
            &actor,
            NewAuditEntry {
                action: "user.login",
                order_id: None,
                client_id: None,
                summary: format!("Вход: {}", u.login),
                details: Some(json!({ "device": device_label.trim() })),
            },
        )
        .await?;
        tx.commit().await?;
        Ok(Some(Session {
            token,
            user: u.into(),
            permissions,
        }))
    }

    async fn record_failed_login(&self, login: &str) -> Result<(), CoreError> {
        // Набранный текст в журнал не попадает (в поле логина могли ввести пароль): пишем только
        // сохранённый логин существующей учётной записи.
        let stored_login = user::Entity::find()
            .filter(user::Column::LoginKey.eq(normalize_search(login)))
            .one(&self.db)
            .await?
            .map(|u| u.login)
            .filter(|l| !l.is_empty());
        let who = stored_login.unwrap_or_else(|| "неизвестный логин".to_owned());
        let tx = self.begin_write().await?;
        let actor = Actor {
            user_id: None,
            name: who.clone(),
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
                summary: format!("Неудачный вход: {who}"),
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
            .filter(user_signs_in)
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
                can_sign_in: true,
                is_master: false,
                created_at: Default::default(),
                version: 1,
            },
            permissions: PermissionSet::empty(),
        };
        let text = format!("{s:?}");
        assert!(text.contains("\"***\"") && !text.contains("abc123"));
    }

    #[test]
    fn dummy_verify_needed_unless_argon2_was_checked() {
        assert!(!needs_dummy_verify(Some(PasswordScheme::Argon2)));
        assert!(needs_dummy_verify(Some(PasswordScheme::LegacySha1)));
        assert!(needs_dummy_verify(None));
    }

    #[tokio::test]
    async fn login_commit_rejects_user_changed_after_verification() {
        use mw_db::entity::user as u;
        for (label, db) in mw_db::testing::all_backends().await {
            let clock = std::sync::Arc::new(crate::FixedClock::new(chrono::Utc::now()));
            let core = Core::open(db, clock, "UTC").await.unwrap();
            let now = core.now();
            let mk = |login: &str, hash: &str, scheme: &str| u::ActiveModel {
                login: Set(login.into()),
                login_key: Set(login.into()),
                display_name: Set(String::new()),
                role_id: Set(1),
                password_hash: Set(hash.into()),
                password_scheme: Set(scheme.into()),
                active: Set(true),
                created_at: Set(now),
                updated_at: Set(now),
                version: Set(1),
                ..Default::default()
            };
            let legacy = mk("old", "aaaa", "legacy_sha1")
                .insert(core.db())
                .await
                .unwrap();
            let modern = mk("new", "bbbb", "argon2").insert(core.db()).await.unwrap();
            // «Верифицированные» модели устарели: пароль сменили, а второго пользователя отключили.
            u::ActiveModel {
                id: Set(legacy.id),
                password_hash: Set("cccc".into()),
                password_scheme: Set("argon2".into()),
                ..Default::default()
            }
            .update(core.db())
            .await
            .unwrap();
            u::ActiveModel {
                id: Set(modern.id),
                active: Set(false),
                ..Default::default()
            }
            .update(core.db())
            .await
            .unwrap();

            let res = core
                .commit_login(legacy.clone(), Some("новый-хэш".into()), "x")
                .await
                .unwrap();
            assert!(res.is_none(), "{label}");
            let res = core.commit_login(modern.clone(), None, "x").await.unwrap();
            assert!(res.is_none(), "{label}");
            let kept = u::Entity::find_by_id(legacy.id)
                .one(core.db())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(kept.password_hash, "cccc", "{label}");
            assert_eq!(
                session::Entity::find().all(core.db()).await.unwrap().len(),
                0
            );
            // Актуальная модель проходит.
            let fresh = u::Entity::find_by_id(legacy.id)
                .one(core.db())
                .await
                .unwrap()
                .unwrap();
            assert!(
                core.commit_login(fresh, None, "x").await.unwrap().is_some(),
                "{label}"
            );
        }
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
