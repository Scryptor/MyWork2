mod common;

use common::*;
use mw_core::{
    Actor, CoreError, NewRole, NewUser, PasswordScheme, RolePatch, UserPatch, hash_password,
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
        // свой пароль можно сменить без права users_manage
        let me = Actor {
            user_id: Some(boss.id),
            ..actor_with(&[])
        };
        t.core.set_password(&me, boss.id, "новый-пароль").await?;
        assert!(matches!(
            t.core.set_password(&me, irina.id, "zzzz").await,
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
