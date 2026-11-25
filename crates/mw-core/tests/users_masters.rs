//! Пользователи без входа, отметка «Мастер», удаление роли, список мастеров.

mod common;

use common::*;
use mw_core::{CoreError, MasterUser, NewRole, NewUser, RolePatch, UserPatch};
use mw_domain::permission::{Permission, PermissionSet};

fn with_sign_in(login: &str, name: &str, role_id: i64, password: &str) -> NewUser {
    NewUser {
        login: login.into(),
        display_name: name.into(),
        role_id,
        password: password.into(),
        can_sign_in: true,
        is_master: false,
    }
}

/// Мастер без входа: ни логина, ни пароля.
fn master_without_sign_in(name: &str, role_id: i64) -> NewUser {
    NewUser {
        login: String::new(),
        display_name: name.into(),
        role_id,
        password: String::new(),
        can_sign_in: false,
        is_master: true,
    }
}

fn patch_sign_in(on: bool) -> UserPatch {
    UserPatch {
        can_sign_in: Some(on),
        ..Default::default()
    }
}

#[tokio::test]
async fn masters_without_sign_in_keep_open_mode() -> Result<(), CoreError> {
    for t in cores().await {
        let master_role = role_id_by_name(&t.core, "Мастер").await;
        let admin_role = role_id_by_name(&t.core, "Администратор").await;
        let petr = t
            .core
            .create_user(&admin(), master_without_sign_in(" Пётр ", master_role))
            .await?;
        let ivan = t
            .core
            .create_user(&admin(), master_without_sign_in("Иван", master_role))
            .await?;
        // Даже в роли администратора пользователь без входа не отменяет режим без входа.
        t.core
            .create_user(&admin(), master_without_sign_in("Шеф", admin_role))
            .await?;
        assert!(t.core.is_open_mode().await?, "{}", t.label);
        assert_eq!(
            (
                petr.login.as_str(),
                petr.display_name.as_str(),
                petr.can_sign_in,
                petr.is_master,
                petr.active
            ),
            ("", "Пётр", false, true, true),
            "{}",
            t.label
        );
        assert_ne!(petr.id, ivan.id);
        assert_eq!(t.core.list_users(&admin()).await?.len(), 3);
        assert!(audit_json(&t.core, "user.created").await.contains("«Пётр»"));
        // Пароль можно задать заранее: войти он всё равно не даст, пока вход выключен.
        t.core.set_password(&admin(), petr.id, "5678").await?;
        assert!(
            audit_json(&t.core, "user.password_changed")
                .await
                .contains("«Пётр»")
        );
        assert!(t.core.is_open_mode().await?);

        // Без логина нужно имя.
        assert_validation(
            t.core
                .create_user(&admin(), master_without_sign_in("  ", master_role))
                .await,
            &[("display_name", "required")],
        );
        // Со входом логин и пароль обязательны, как раньше.
        assert_validation(
            t.core
                .create_user(&admin(), with_sign_in(" ", "Анна", admin_role, ""))
                .await,
            &[("login", "required"), ("password", "too_short")],
        );
        // Первый пользователь со входом — по-прежнему только администратор.
        assert_validation(
            t.core
                .create_user(&admin(), with_sign_in("kassa", "", master_role, "1234"))
                .await,
            &[("role_id", "last_admin")],
        );
        assert!(t.core.is_open_mode().await?);
        t.core
            .create_user(&admin(), with_sign_in("boss", "", admin_role, "1234"))
            .await?;
        assert!(!t.core.is_open_mode().await?);
    }
    Ok(())
}

#[tokio::test]
async fn users_without_sign_in_do_not_keep_admin_access() -> Result<(), CoreError> {
    for t in cores().await {
        let admin_role = role_id_by_name(&t.core, "Администратор").await;
        let boss = t
            .core
            .create_user(&admin(), with_sign_in("boss", "Шеф", admin_role, "1234"))
            .await?;
        // У администратора без входа права есть, но войти и вернуть доступ он не может.
        t.core
            .create_user(&admin(), master_without_sign_in("Зам", admin_role))
            .await?;
        assert_validation(
            t.core
                .update_user(&admin(), boss.id, 1, patch_sign_in(false))
                .await,
            &[("can_sign_in", "last_admin")],
        );
        assert_validation(
            t.core
                .update_user(
                    &admin(),
                    boss.id,
                    1,
                    UserPatch {
                        active: Some(false),
                        ..Default::default()
                    },
                )
                .await,
            &[("active", "last_admin")],
        );
        let seniors = t
            .core
            .create_role(
                &admin(),
                NewRole {
                    name: "Старшие".into(),
                    permissions: PermissionSet::from_codes(["users_manage"]),
                },
            )
            .await?;
        t.core
            .create_user(&admin(), master_without_sign_in("Старший", seniors.id))
            .await?;
        let only_orders = RolePatch {
            permissions: Some(PermissionSet::from_codes(["orders_create"])),
            ..Default::default()
        };
        assert_validation(
            t.core
                .update_role(&admin(), admin_role, 2, only_orders.clone())
                .await,
            &[("permissions", "last_admin")],
        );
        // Со вторым администратором, который входит, — можно.
        t.core
            .create_user(&admin(), with_sign_in("deputy", "", seniors.id, "1234"))
            .await?;
        let boss = t
            .core
            .update_user(&admin(), boss.id, 1, patch_sign_in(false))
            .await?;
        assert!(!boss.can_sign_in, "{}", t.label);
        t.core
            .update_role(&admin(), admin_role, 2, only_orders)
            .await?;
        assert!(!t.core.is_open_mode().await?);
    }
    Ok(())
}

#[tokio::test]
async fn enabling_sign_in_needs_login_and_password_and_disabling_ends_sessions()
-> Result<(), CoreError> {
    for t in cores().await {
        let admin_role = role_id_by_name(&t.core, "Администратор").await;
        let master_role = role_id_by_name(&t.core, "Мастер").await;
        t.core
            .create_user(&admin(), with_sign_in("boss", "Шеф", admin_role, "1234"))
            .await?;
        let petr = t
            .core
            .create_user(&admin(), master_without_sign_in("Пётр", master_role))
            .await?;
        let enable = |login: &str, password: &str| UserPatch {
            can_sign_in: Some(true),
            login: Some(login.into()),
            password: Some(password.into()),
            ..Default::default()
        };
        assert_validation(
            t.core
                .update_user(&admin(), petr.id, 1, patch_sign_in(true))
                .await,
            &[("login", "required"), ("password", "required")],
        );
        assert_validation(
            t.core
                .update_user(&admin(), petr.id, 1, enable(" BOSS ", "5678"))
                .await,
            &[("login", "duplicate")],
        );
        assert_validation(
            t.core
                .update_user(&admin(), petr.id, 1, enable("petr", "12"))
                .await,
            &[("password", "too_short")],
        );
        let petr = t
            .core
            .update_user(&admin(), petr.id, 1, enable(" Petr ", "5678"))
            .await?;
        assert_eq!(
            (petr.login.as_str(), petr.can_sign_in, petr.version),
            ("Petr", true, 2),
            "{}",
            t.label
        );
        let session = t.core.login("petr", "5678", "x").await?;
        assert!(!audit_json(&t.core, "user.updated").await.contains("5678"));

        // Вход выключен: сеансы закрыты, верный пароль не помогает; логин и пароль сохраняются.
        let petr = t
            .core
            .update_user(&admin(), petr.id, 2, patch_sign_in(false))
            .await?;
        assert_eq!((petr.login.as_str(), petr.can_sign_in), ("Petr", false));
        assert!(matches!(
            t.core.authenticate(&session.token).await,
            Err(CoreError::Unauthenticated)
        ));
        assert!(matches!(
            t.core.login("petr", "5678", "x").await,
            Err(CoreError::InvalidCredentials)
        ));
        // Снова включить — прежние логин и пароль подходят.
        t.core
            .update_user(&admin(), petr.id, 3, patch_sign_in(true))
            .await?;
        t.core.login("petr", "5678", "x").await?;
        // Пароль в правке закрывает сеансы, как сброс пароля.
        let session = t.core.login("petr", "5678", "x").await?;
        t.core
            .update_user(
                &admin(),
                petr.id,
                4,
                UserPatch {
                    password: Some("9999".into()),
                    ..Default::default()
                },
            )
            .await?;
        assert!(t.core.authenticate(&session.token).await.is_err());
        t.core.login("petr", "9999", "x").await?;
        assert!(!audit_json(&t.core, "user.updated").await.contains("9999"));
    }
    Ok(())
}

#[tokio::test]
async fn user_without_sign_in_is_refused_like_a_wrong_password() -> Result<(), CoreError> {
    for t in cores().await {
        let admin_role = role_id_by_name(&t.core, "Администратор").await;
        t.core
            .create_user(&admin(), with_sign_in("boss", "Шеф", admin_role, "1234"))
            .await?;
        let ivan = t
            .core
            .create_user(
                &admin(),
                NewUser {
                    can_sign_in: false,
                    is_master: true,
                    ..with_sign_in("ivan", "Иван", admin_role, "1234")
                },
            )
            .await?;
        assert_eq!((ivan.login.as_str(), ivan.can_sign_in), ("ivan", false));
        for (login, password) in [("ivan", "1234"), ("", ""), ("ivan", "")] {
            assert!(
                matches!(
                    t.core.login(login, password, "x").await,
                    Err(CoreError::InvalidCredentials)
                ),
                "{}: {login}",
                t.label
            );
        }
        assert_eq!(count_audit(&t.core, "user.login_failed").await, 3);
        assert_eq!(count_audit(&t.core, "user.login").await, 0);
    }
    Ok(())
}

#[tokio::test]
async fn unused_role_is_deleted_and_role_with_users_is_kept() -> Result<(), CoreError> {
    for t in cores().await {
        let stock = t
            .core
            .create_role(
                &admin(),
                NewRole {
                    name: "Склад".into(),
                    permissions: PermissionSet::from_codes(["stock_view"]),
                },
            )
            .await?;
        assert!(matches!(
            t.core
                .delete_role(&actor_with(&[Permission::OrdersCreate]), stock.id, 1)
                .await,
            Err(CoreError::Forbidden(Permission::UsersManage))
        ));
        assert!(matches!(
            t.core.delete_role(&admin(), stock.id, 2).await,
            Err(CoreError::Conflict {
                current_version: 1,
                ..
            })
        ));
        let master_role = role_id_by_name(&t.core, "Мастер").await;
        t.core
            .create_user(&admin(), master_without_sign_in("Пётр", master_role))
            .await?;
        let disabled = t
            .core
            .create_user(&admin(), master_without_sign_in("Иван", master_role))
            .await?;
        t.core
            .update_user(
                &admin(),
                disabled.id,
                1,
                UserPatch {
                    active: Some(false),
                    ..Default::default()
                },
            )
            .await?;
        // Версия готовой роли — не 1: миграция склада дала ей просмотр склада.
        let master_version = t
            .core
            .list_roles(&admin())
            .await?
            .into_iter()
            .find(|r| r.id == master_role)
            .map(|r| r.version)
            .unwrap();
        match t
            .core
            .delete_role(&admin(), master_role, master_version)
            .await
        {
            Err(CoreError::InUse(message)) => {
                assert_eq!(message, "В роли есть пользователи: 2", "{}", t.label)
            }
            other => panic!("{}: ожидался отказ, получено {other:?}", t.label),
        }
        t.core.delete_role(&admin(), stock.id, 1).await?;
        assert!(matches!(
            t.core.delete_role(&admin(), stock.id, 1).await,
            Err(CoreError::NotFound { .. })
        ));
        let names: Vec<String> = t
            .core
            .list_roles(&admin())
            .await?
            .into_iter()
            .map(|r| r.name)
            .collect();
        assert_eq!(
            names,
            ["Администратор", "Мастер", "Приёмщик"],
            "{}",
            t.label
        );
        assert_eq!(count_audit(&t.core, "role.deleted").await, 1);
        assert!(
            audit_json(&t.core, "role.deleted")
                .await
                .contains("«Склад»")
        );
    }
    Ok(())
}

#[tokio::test]
async fn masters_list_active_first_then_by_name() -> Result<(), CoreError> {
    for t in cores().await {
        let admin_role = role_id_by_name(&t.core, "Администратор").await;
        let master_role = role_id_by_name(&t.core, "Мастер").await;
        // Мастер со входом без отображаемого имени виден по логину.
        let boss = t
            .core
            .create_user(
                &admin(),
                NewUser {
                    is_master: true,
                    ..with_sign_in("boss", "", admin_role, "1234")
                },
            )
            .await?;
        let boris = t
            .core
            .create_user(&admin(), master_without_sign_in("борис", master_role))
            .await?;
        let anna = t
            .core
            .create_user(&admin(), master_without_sign_in("Анна", master_role))
            .await?;
        let vera = t
            .core
            .create_user(&admin(), master_without_sign_in("Вера", master_role))
            .await?;
        let alla = t
            .core
            .create_user(&admin(), master_without_sign_in("Алла", master_role))
            .await?;
        for id in [vera.id, alla.id] {
            t.core
                .update_user(
                    &admin(),
                    id,
                    1,
                    UserPatch {
                        active: Some(false),
                        ..Default::default()
                    },
                )
                .await?;
        }
        // Не мастер в список не попадает; снятая отметка тоже убирает из списка.
        t.core
            .create_user(
                &admin(),
                NewUser {
                    is_master: false,
                    ..master_without_sign_in("Гость", master_role)
                },
            )
            .await?;
        let gleb = t
            .core
            .create_user(&admin(), master_without_sign_in("Глеб", master_role))
            .await?;
        t.core
            .update_user(
                &admin(),
                gleb.id,
                1,
                UserPatch {
                    is_master: Some(false),
                    ..Default::default()
                },
            )
            .await?;

        // Любой вошедший, без особых прав.
        let masters = t.core.list_masters(&actor_with(&[])).await?;
        let expected = [
            (boss.id, "boss", true),
            (anna.id, "Анна", true),
            (boris.id, "борис", true),
            (alla.id, "Алла", false),
            (vera.id, "Вера", false),
        ]
        .map(|(id, name, active)| MasterUser {
            id,
            display_name: name.into(),
            active,
        });
        assert_eq!(masters, expected, "{}", t.label);
    }
    Ok(())
}

#[tokio::test]
async fn imported_user_without_password_stays_editable() -> Result<(), CoreError> {
    for t in cores().await {
        let admin_role = role_id_by_name(&t.core, "Администратор").await;
        let master_role = role_id_by_name(&t.core, "Мастер").await;
        let boss = t
            .core
            .create_user(&admin(), with_sign_in("boss", "Шеф", admin_role, "1234"))
            .await?;
        // Как после импорта MyWork 2 с непереносимым паролем: вход включён, хэша нет.
        insert_legacy_user(&t.core, "ира", "", admin_role).await;
        let ira = t
            .core
            .list_users(&admin())
            .await?
            .into_iter()
            .find(|u| u.login == "ира")
            .unwrap();
        assert!(ira.can_sign_in, "{}", t.label);
        let mut version = ira.version;
        for patch in [
            UserPatch {
                display_name: Some("Ира".into()),
                ..Default::default()
            },
            UserPatch {
                is_master: Some(true),
                ..Default::default()
            },
            UserPatch {
                role_id: Some(master_role),
                ..Default::default()
            },
            UserPatch {
                login: Some("Ирина".into()),
                ..Default::default()
            },
            UserPatch {
                active: Some(false),
                ..Default::default()
            },
            UserPatch {
                active: Some(true),
                ..Default::default()
            },
            patch_sign_in(false),
        ] {
            let label = format!("{}: {patch:?}", t.label);
            version = t
                .core
                .update_user(&admin(), ira.id, version, patch)
                .await
                .unwrap_or_else(|e| panic!("{label}: {e:?}"))
                .version;
        }
        // Снова включить вход без пароля нельзя.
        assert_validation(
            t.core
                .update_user(&admin(), ira.id, version, patch_sign_in(true))
                .await,
            &[("password", "required")],
        );
        // Убрать логин у пользователя со входом нельзя.
        assert_validation(
            t.core
                .update_user(
                    &admin(),
                    boss.id,
                    1,
                    UserPatch {
                        login: Some(" ".into()),
                        ..Default::default()
                    },
                )
                .await,
            &[("login", "required")],
        );
    }
    Ok(())
}

#[tokio::test]
async fn last_role_with_users_manage_is_not_deleted() -> Result<(), CoreError> {
    const LAST: &str = "Нельзя удалить последнюю роль с правом «Пользователи и права»";
    for t in cores().await {
        let admin_role = role_id_by_name(&t.core, "Администратор").await;
        // Свежая база в режиме без входа: пользователей нет, но без этой роли не создать администратора.
        assert!(t.core.is_open_mode().await?);
        match t.core.delete_role(&admin(), admin_role, 2).await {
            Err(CoreError::InUse(message)) => assert_eq!(message, LAST, "{}", t.label),
            other => panic!("{}: ожидался отказ, получено {other:?}", t.label),
        }
        let chiefs = t
            .core
            .create_role(
                &admin(),
                NewRole {
                    name: "Начальство".into(),
                    permissions: PermissionSet::from_codes(["users_manage", "audit_view"]),
                },
            )
            .await?;
        t.core.delete_role(&admin(), admin_role, 2).await?;
        match t.core.delete_role(&admin(), chiefs.id, 1).await {
            Err(CoreError::InUse(message)) => assert_eq!(message, LAST, "{}", t.label),
            other => panic!("{}: ожидался отказ, получено {other:?}", t.label),
        }
        assert!(
            t.core
                .list_roles(&admin())
                .await?
                .iter()
                .any(|r| r.permissions.contains(Permission::UsersManage))
        );
    }
    Ok(())
}

#[tokio::test]
async fn last_role_keeps_users_manage() -> Result<(), CoreError> {
    const LAST: &str = "Нельзя снять право «Пользователи и права» с последней роли, где оно есть";
    let only_orders = || RolePatch {
        permissions: Some(PermissionSet::from_codes(["orders_create"])),
        ..Default::default()
    };
    for t in cores().await {
        let admin_role = role_id_by_name(&t.core, "Администратор").await;
        // Режим без входа: пользователей нет, защита от потери доступа молчит — но роль с правом нужна.
        assert!(t.core.is_open_mode().await?);
        match t
            .core
            .update_role(&admin(), admin_role, 2, only_orders())
            .await
        {
            Err(CoreError::InUse(message)) => assert_eq!(message, LAST, "{}", t.label),
            other => panic!("{}: ожидался отказ, получено {other:?}", t.label),
        }
        // Переименовать последнюю роль с правом можно.
        t.core
            .update_role(
                &admin(),
                admin_role,
                2,
                RolePatch {
                    name: Some("Админы".into()),
                    ..Default::default()
                },
            )
            .await?;
        let chiefs = t
            .core
            .create_role(
                &admin(),
                NewRole {
                    name: "Начальство".into(),
                    permissions: PermissionSet::from_codes(["users_manage"]),
                },
            )
            .await?;
        t.core
            .update_role(&admin(), admin_role, 3, only_orders())
            .await?;
        match t
            .core
            .update_role(&admin(), chiefs.id, 1, only_orders())
            .await
        {
            Err(CoreError::InUse(message)) => assert_eq!(message, LAST, "{}", t.label),
            other => panic!("{}: ожидался отказ, получено {other:?}", t.label),
        }
    }
    Ok(())
}

#[tokio::test]
async fn users_without_password_cannot_sign_in_and_do_not_count() -> Result<(), CoreError> {
    for t in cores().await {
        let admin_role = role_id_by_name(&t.core, "Администратор").await;
        let clerk_role = role_id_by_name(&t.core, "Приёмщик").await;
        // Как после импорта MyWork 2 с непереносимыми паролями: вход включён, хэшей нет.
        insert_legacy_user(&t.core, "ира", "", admin_role).await;
        insert_legacy_user(&t.core, "kassa", "", clerk_role).await;
        let by_login = |users: Vec<mw_core::User>, login: &str| {
            users.into_iter().find(|u| u.login == login).unwrap()
        };
        let users = t.core.list_users(&admin()).await?;
        let (ira, kassa) = (by_login(users.clone(), "ира"), by_login(users, "kassa"));
        assert!(t.core.is_open_mode().await?, "{}", t.label);

        // Путь 1: отключить и снова включить администратора без пароля — режим без входа остаётся.
        for (version, active) in [(1, false), (2, true)] {
            t.core
                .update_user(
                    &admin(),
                    ira.id,
                    version,
                    UserPatch {
                        active: Some(active),
                        ..Default::default()
                    },
                )
                .await?;
            assert!(t.core.is_open_mode().await?, "{}: {active}", t.label);
        }
        assert!(matches!(
            t.core.login("ира", "", "x").await,
            Err(CoreError::InvalidCredentials)
        ));

        // Пароль приёмщику первым — режим без входа кончился бы без администратора.
        assert_validation(
            t.core.set_password(&admin(), kassa.id, "1234").await,
            &[("password", "last_admin")],
        );
        assert_validation(
            t.core
                .update_user(
                    &admin(),
                    kassa.id,
                    1,
                    UserPatch {
                        password: Some("1234".into()),
                        ..Default::default()
                    },
                )
                .await,
            &[("password", "last_admin")],
        );
        assert!(t.core.is_open_mode().await?);
        // Пароль администратору — режим без входа кончается, войти можно.
        t.core.set_password(&admin(), ira.id, "5678").await?;
        assert!(!t.core.is_open_mode().await?, "{}", t.label);
        t.core.login("ира", "5678", "x").await?;
        t.core.set_password(&admin(), kassa.id, "1234").await?;

        // Путь 2: администратор без пароля не заменяет настоящего.
        insert_legacy_user(&t.core, "zam", "", admin_role).await;
        assert_validation(
            t.core
                .update_user(
                    &admin(),
                    ira.id,
                    3,
                    UserPatch {
                        active: Some(false),
                        ..Default::default()
                    },
                )
                .await,
            &[("active", "last_admin")],
        );
        assert_validation(
            t.core
                .update_user(&admin(), ira.id, 3, patch_sign_in(false))
                .await,
            &[("can_sign_in", "last_admin")],
        );
    }
    Ok(())
}
