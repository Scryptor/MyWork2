mod common;

use common::*;
use mw_core::{
    CoreError, NewRole, NewUser, PasswordScheme, RolePatch, UserPatch, hash_password,
    legacy_sha1_hex, verify_password,
};
use mw_domain::permission::{Permission, PermissionSet};

#[test]
fn legacy_sha1_matches_mywork2() {
    assert_eq!(
        legacy_sha1_hex("1234"),
        "139f69c93c042496a8e958ec5930662c6cccafbf"
    );
    assert_eq!(
        legacy_sha1_hex("пароль"),
        "cb72cc1440f262fa00fc0223c1f561d50cc061aa"
    );
    assert!(verify_password(
        PasswordScheme::LegacySha1,
        "139f69c93c042496a8e958ec5930662c6cccafbf",
        "1234"
    ));
}

#[test]
fn argon2_hashes_are_salted_and_verify() {
    let (h1, h2) = (
        hash_password("secret").unwrap(),
        hash_password("secret").unwrap(),
    );
    assert_ne!(h1, h2);
    assert!(verify_password(PasswordScheme::Argon2, &h1, "secret"));
    assert!(!verify_password(PasswordScheme::Argon2, &h1, "Secret"));
}

#[tokio::test]
async fn users_validation_and_last_admin_guard() -> Result<(), CoreError> {
    for t in cores().await {
        assert!(t.core.is_open_mode().await?, "{}", t.label);
        let admin_role = role_id_by_name(&t.core, "Администратор").await;
        let irina = t
            .core
            .create_user(
                &admin(),
                NewUser {
                    login: "Ирина".into(),
                    display_name: "Ирина".into(),
                    role_id: admin_role,
                    password: "1234".into(),
                },
            )
            .await?;
        assert!(!t.core.is_open_mode().await?);
        assert_validation(
            t.core
                .create_user(
                    &admin(),
                    NewUser {
                        login: " ирина ".into(),
                        display_name: String::new(),
                        role_id: admin_role,
                        password: "12".into(),
                    },
                )
                .await,
            &[("login", "duplicate"), ("password", "too_short")],
        );
        assert_validation(
            t.core
                .update_user(
                    &admin(),
                    irina.id,
                    1,
                    UserPatch {
                        active: Some(false),
                        ..Default::default()
                    },
                )
                .await,
            &[("active", "last_admin")],
        );
        let clerk_role = t
            .core
            .create_role(
                &admin(),
                NewRole {
                    name: "Приёмщик".into(),
                    permissions: PermissionSet::from_codes(["orders_create"]),
                },
            )
            .await?;
        assert_validation(
            t.core
                .update_user(
                    &admin(),
                    irina.id,
                    1,
                    UserPatch {
                        role_id: Some(clerk_role.id),
                        ..Default::default()
                    },
                )
                .await,
            &[("role_id", "last_admin")],
        );
        assert_validation(
            t.core
                .update_role(
                    &admin(),
                    admin_role,
                    1,
                    RolePatch {
                        permissions: Some(PermissionSet::from_codes(["orders_create"])),
                        ..Default::default()
                    },
                )
                .await,
            &[("permissions", "last_admin")],
        );
        let boss = t
            .core
            .create_user(
                &admin(),
                NewUser {
                    login: "boss".into(),
                    display_name: "Шеф".into(),
                    role_id: admin_role,
                    password: "qwerty".into(),
                },
            )
            .await?;
        assert!(
            t.core
                .update_user(
                    &admin(),
                    irina.id,
                    1,
                    UserPatch {
                        active: Some(false),
                        ..Default::default()
                    },
                )
                .await
                .is_ok()
        );
        // свой пароль меняют только с текущим паролем, без права users_manage
        let me = user_actor(boss.id);
        t.core
            .change_own_password(&me, "qwerty", "новый-пароль")
            .await?;
        assert!(matches!(
            t.core.set_password(&me, irina.id, "zzzz").await,
            Err(CoreError::Forbidden(Permission::UsersManage))
        ));
        assert!(matches!(
            t.core.set_password(&me, boss.id, "zzzz").await,
            Err(CoreError::Forbidden(Permission::UsersManage))
        ));
        assert!(
            !audit_json(&t.core, "user.password_changed")
                .await
                .contains("новый-пароль")
        );
    }
    Ok(())
}

#[tokio::test]
async fn first_user_without_users_manage_is_refused_in_open_mode() -> Result<(), CoreError> {
    for t in cores().await {
        let clerk_role = t
            .core
            .create_role(
                &admin(),
                NewRole {
                    name: "Приёмщик".into(),
                    permissions: PermissionSet::from_codes(["orders_create"]),
                },
            )
            .await?;
        assert_validation(
            t.core
                .create_user(
                    &admin(),
                    NewUser {
                        login: "kassa".into(),
                        display_name: String::new(),
                        role_id: clerk_role.id,
                        password: "1234".into(),
                    },
                )
                .await,
            &[("role_id", "last_admin")],
        );
        assert!(t.core.is_open_mode().await?, "{}", t.label);
        let admin_role = role_id_by_name(&t.core, "Администратор").await;
        t.core
            .create_user(
                &admin(),
                NewUser {
                    login: "boss".into(),
                    display_name: String::new(),
                    role_id: admin_role,
                    password: "1234".into(),
                },
            )
            .await?;
        // С администратором в системе пользователь без прав создаётся свободно.
        t.core
            .create_user(
                &admin(),
                NewUser {
                    login: "kassa".into(),
                    display_name: String::new(),
                    role_id: clerk_role.id,
                    password: "1234".into(),
                },
            )
            .await?;
    }
    Ok(())
}

#[tokio::test]
async fn change_own_password_requires_current_password() -> Result<(), CoreError> {
    for t in cores().await {
        let role = role_id_by_name(&t.core, "Администратор").await;
        let u = t
            .core
            .create_user(
                &admin(),
                NewUser {
                    login: "анна".into(),
                    display_name: String::new(),
                    role_id: role,
                    password: "1234".into(),
                },
            )
            .await?;
        let me = user_actor(u.id);
        let session = t.core.login("анна", "1234", "x").await?;

        assert_validation(
            t.core
                .change_own_password(&me, "wrong", "новый-пароль")
                .await,
            &[("current_password", "invalid")],
        );
        t.core.login("анна", "1234", "x").await?;
        t.core.authenticate(&session.token).await?;
        assert_validation(
            t.core.change_own_password(&me, "1234", "12").await,
            &[("new_password", "too_short")],
        );
        assert!(matches!(
            t.core
                .change_own_password(&actor_with(&[]), "1234", "новый-пароль")
                .await,
            Err(CoreError::Unauthenticated)
        ));

        t.core
            .change_own_password(&me, "1234", "новый-пароль")
            .await?;
        t.core.login("анна", "новый-пароль", "x").await?;
        assert!(
            matches!(
                t.core.login("анна", "1234", "x").await,
                Err(CoreError::InvalidCredentials)
            ),
            "{}",
            t.label
        );
        assert!(matches!(
            t.core.authenticate(&session.token).await,
            Err(CoreError::Unauthenticated)
        ));
        let log = audit_json(&t.core, "user.password_changed").await;
        assert!(!log.contains("новый-пароль") && !log.contains("1234"));
    }
    Ok(())
}

#[tokio::test]
async fn broken_database_without_admin_allows_unrelated_edits() -> Result<(), CoreError> {
    for t in cores().await {
        // Как после импорта: активные пользователи есть, права users_manage нет ни у кого.
        let clerk = t
            .core
            .create_role(
                &admin(),
                NewRole {
                    name: "Приёмщик".into(),
                    permissions: PermissionSet::from_codes(["orders_create"]),
                },
            )
            .await?
            .id;
        let kassa = insert_user_row(&t.core, "kassa", clerk, true).await;
        let sklad = insert_user_row(&t.core, "sklad", clerk, true).await;
        t.core
            .update_user(
                &admin(),
                kassa,
                1,
                UserPatch {
                    display_name: Some("Касса".into()),
                    ..Default::default()
                },
            )
            .await?;
        t.core
            .update_user(
                &admin(),
                sklad,
                1,
                UserPatch {
                    active: Some(false),
                    ..Default::default()
                },
            )
            .await?;
        t.core
            .create_user(
                &admin(),
                NewUser {
                    login: "master".into(),
                    display_name: String::new(),
                    role_id: clerk,
                    password: "1234".into(),
                },
            )
            .await?;
    }
    Ok(())
}

#[tokio::test]
async fn enabling_user_without_rights_in_open_mode_is_refused() -> Result<(), CoreError> {
    for t in cores().await {
        let clerk = t
            .core
            .create_role(
                &admin(),
                NewRole {
                    name: "Приёмщик".into(),
                    permissions: PermissionSet::from_codes(["orders_create"]),
                },
            )
            .await?
            .id;
        let kassa = insert_user_row(&t.core, "kassa", clerk, false).await;
        assert!(t.core.is_open_mode().await?);
        assert_validation(
            t.core
                .update_user(
                    &admin(),
                    kassa,
                    1,
                    UserPatch {
                        active: Some(true),
                        ..Default::default()
                    },
                )
                .await,
            &[("active", "last_admin")],
        );
        let boss = insert_user_row(
            &t.core,
            "boss",
            role_id_by_name(&t.core, "Администратор").await,
            false,
        )
        .await;
        t.core
            .update_user(
                &admin(),
                boss,
                1,
                UserPatch {
                    active: Some(true),
                    ..Default::default()
                },
            )
            .await?;
        assert!(!t.core.is_open_mode().await?);
    }
    Ok(())
}
