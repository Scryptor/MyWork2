mod common;

use chrono::Duration;
use common::*;
use mw_core::{CoreError, NewRole, NewUser, UserPatch};
use mw_domain::permission::{Permission, PermissionSet};

#[tokio::test]
async fn login_sessions_and_legacy_upgrade() {
    for t in cores().await {
        let role = role_id_by_name(&t.core, "Администратор").await;
        let u = t
            .core
            .create_user(
                &admin(),
                NewUser {
                    login: "Ирина".into(),
                    display_name: "Ирина П.".into(),
                    role_id: role,
                    password: "1234".into(),
                    can_sign_in: true,
                    is_master: false,
                },
            )
            .await
            .unwrap();
        let s = t.core.login("ирина", "1234", "Касса").await.unwrap();
        assert_eq!((s.token.len(), s.user.id), (64, u.id), "{}", t.label);
        assert!(
            stored_token_hashes(&t.core)
                .await
                .iter()
                .all(|h| h != &s.token)
        );
        assert_eq!(count_audit(&t.core, "user.login").await, 1);
        for (login, pass) in [("ирина", "12345"), ("никто", "1234")] {
            assert!(matches!(
                t.core.login(login, pass, "x").await,
                Err(CoreError::InvalidCredentials)
            ));
        }
        assert_eq!(count_audit(&t.core, "user.login_failed").await, 2);
        let failed = audit_json(&t.core, "user.login_failed").await;
        assert!(failed.contains("Неудачный вход: Ирина"));
        assert!(failed.contains("Неудачный вход: неизвестный логин"));
        assert!(!failed.contains("никто") && !failed.contains("12345"));

        let actor = t.core.authenticate(&s.token).await.unwrap();
        assert_eq!((actor.user_id(), actor.name()), (Some(u.id), "Ирина П."));
        assert!(matches!(
            t.core.authenticate("нет-такого").await,
            Err(CoreError::Unauthenticated)
        ));

        // сессия живёт, пока ей пользуются
        t.clock.advance(Duration::days(20));
        t.core.authenticate(&s.token).await.unwrap();
        t.clock.advance(Duration::days(25));
        t.core.authenticate(&s.token).await.unwrap();
        t.clock.advance(Duration::days(31));
        assert!(matches!(
            t.core.authenticate(&s.token).await,
            Err(CoreError::Unauthenticated)
        ));

        // выход
        let s2 = t.core.login("Ирина", "1234", "Телефон").await.unwrap();
        t.core.logout(&s2.token).await.unwrap();
        t.core.logout(&s2.token).await.unwrap();
        assert!(matches!(
            t.core.authenticate(&s2.token).await,
            Err(CoreError::Unauthenticated)
        ));

        // старый пароль из MyWork 2 перехэшируется при первом входе
        insert_legacy_user(
            &t.core,
            "старый",
            "139f69c93c042496a8e958ec5930662c6cccafbf",
            role,
        )
        .await;
        t.core.login("старый", "1234", "x").await.unwrap();
        assert_eq!(user_scheme(&t.core, "старый").await, "argon2");
        t.core.login("старый", "1234", "x").await.unwrap();

        // права роли действуют сразу, без перевхода
        let s3 = t.core.login("Ирина", "1234", "x").await.unwrap();
        t.core
            .create_user(
                &admin(),
                NewUser {
                    login: "boss".into(),
                    display_name: String::new(),
                    role_id: role,
                    password: "qwerty".into(),
                    can_sign_in: true,
                    is_master: false,
                },
            )
            .await
            .unwrap();
        let clerk = t
            .core
            .create_role(
                &admin(),
                NewRole {
                    name: "Продавец".into(),
                    permissions: PermissionSet::from_codes(["orders_create"]),
                },
            )
            .await
            .unwrap();
        t.core
            .update_user(
                &admin(),
                u.id,
                1,
                UserPatch {
                    role_id: Some(clerk.id),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert!(
            !t.core
                .authenticate(&s3.token)
                .await
                .unwrap()
                .permissions()
                .contains(Permission::UsersManage)
        );
        let boss = t.core.login("boss", "qwerty", "x").await.unwrap();
        assert_eq!(
            t.core.authenticate(&boss.token).await.unwrap().name(),
            "boss"
        );
    }
}

#[tokio::test]
async fn password_change_and_deactivation_revoke_sessions() {
    for t in cores().await {
        let role = role_id_by_name(&t.core, "Администратор").await;
        let new_user = |login: &str| NewUser {
            login: login.into(),
            display_name: String::new(),
            role_id: role,
            password: "1234".into(),
            can_sign_in: true,
            is_master: false,
        };
        let a = t
            .core
            .create_user(&admin(), new_user("анна"))
            .await
            .unwrap();
        let b = t
            .core
            .create_user(&admin(), new_user("борис"))
            .await
            .unwrap();

        let sa = t.core.login("анна", "1234", "x").await.unwrap();
        t.core.authenticate(&sa.token).await.unwrap();
        t.core
            .set_password(&admin(), a.id, "новый-пароль")
            .await
            .unwrap();
        assert!(
            matches!(
                t.core.authenticate(&sa.token).await,
                Err(CoreError::Unauthenticated)
            ),
            "{}",
            t.label
        );
        t.core.login("анна", "новый-пароль", "x").await.unwrap();

        let sb = t.core.login("борис", "1234", "x").await.unwrap();
        let off = UserPatch {
            active: Some(false),
            ..Default::default()
        };
        let b2 = t.core.update_user(&admin(), b.id, 1, off).await.unwrap();
        let on = UserPatch {
            active: Some(true),
            ..Default::default()
        };
        t.core
            .update_user(&admin(), b.id, b2.version, on)
            .await
            .unwrap();
        assert!(
            matches!(
                t.core.authenticate(&sb.token).await,
                Err(CoreError::Unauthenticated)
            ),
            "{}",
            t.label
        );
    }
}

#[tokio::test]
async fn failed_login_never_stores_typed_text() {
    use mw_db::entity::audit_log;
    use sea_orm::EntityTrait;
    for t in cores().await {
        assert!(matches!(
            t.core.login("секрет123", "x", "x").await,
            Err(CoreError::InvalidCredentials)
        ));
        let rows = audit_log::Entity::find().all(t.core.db()).await.unwrap();
        assert_eq!(rows.len(), 1, "{}", t.label);
        let row = &rows[0];
        assert_eq!(row.summary, "Неудачный вход: неизвестный логин");
        assert!(row.details.is_none());
        assert!(!row.user_name.contains("секрет123"), "{}", t.label);
    }
}
