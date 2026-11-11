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
        assert!(failed.contains("Неудачный вход: никто"));
        assert!(!failed.contains("12345"));

        let actor = t.core.authenticate(&s.token).await.unwrap();
        assert_eq!(
            (actor.user_id, actor.name.as_str()),
            (Some(u.id), "Ирина П.")
        );
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
                },
            )
            .await
            .unwrap();
        let clerk = t
            .core
            .create_role(
                &admin(),
                NewRole {
                    name: "Приёмщик".into(),
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
                .permissions
                .contains(Permission::UsersManage)
        );
        let boss = t.core.login("boss", "qwerty", "x").await.unwrap();
        assert_eq!(t.core.authenticate(&boss.token).await.unwrap().name, "boss");
    }
}
