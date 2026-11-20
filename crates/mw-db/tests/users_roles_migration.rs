//! Пользователи без входа, отметка «Мастер» и готовые роли «Мастер» и «Приёмщик» на уже заполненной базе.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mw_db::testing::{all_backends, all_backends_up_to};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};

/// Миграций до новой схемы пользователей (`can_sign_in`, `is_master`).
const BEFORE_USER_FLAGS: u32 = 7;
/// Миграций до готовых ролей.
const BEFORE_BUILTIN_ROLES: u32 = 8;

const MASTER_PERMISSIONS: &str = r#"["orders_edit","clients_view"]"#;
const CLERK_PERMISSIONS: &str =
    r#"["orders_create","orders_edit","clients_view","clients_create","clients_edit"]"#;

async fn exec(db: &DatabaseConnection, sql: &str) {
    db.execute_raw(Statement::from_string(db.get_database_backend(), sql))
        .await
        .unwrap();
}

/// Название роли → (права, версия).
async fn roles(db: &DatabaseConnection) -> BTreeMap<String, (String, i64)> {
    let sql = "SELECT name, permissions, version FROM roles";
    db.query_all_raw(Statement::from_string(db.get_database_backend(), sql))
        .await
        .unwrap()
        .into_iter()
        .map(|r| {
            (
                r.try_get_by_index(0).unwrap(),
                (
                    r.try_get_by_index(1).unwrap(),
                    r.try_get_by_index(2).unwrap(),
                ),
            )
        })
        .collect()
}

#[tokio::test]
async fn new_base_has_builtin_master_and_clerk_roles() {
    for (label, db) in all_backends().await {
        let roles = roles(&db).await;
        assert_eq!(
            roles.keys().map(String::as_str).collect::<Vec<_>>(),
            ["Администратор", "Мастер", "Приёмщик"],
            "{label}"
        );
        assert_eq!(
            roles["Мастер"],
            (MASTER_PERMISSIONS.to_owned(), 1),
            "{label}"
        );
        assert_eq!(
            roles["Приёмщик"],
            (CLERK_PERMISSIONS.to_owned(), 1),
            "{label}"
        );
    }
}

#[tokio::test]
async fn existing_role_with_the_same_name_is_neither_doubled_nor_changed() {
    for (label, db) in all_backends_up_to(BEFORE_BUILTIN_ROLES).await {
        // Как после импорта MyWork 2: роли с такими названиями уже есть, в другом регистре и с «е».
        exec(
            &db,
            r#"INSERT INTO roles (name, permissions, version) VALUES ('мастер', '["orders_create"]', 4)"#,
        )
        .await;
        exec(
            &db,
            r#"INSERT INTO roles (name, permissions, version) VALUES ('ПРИЕМЩИК', '[]', 2)"#,
        )
        .await;
        mw_db::migrate(&db).await.unwrap();
        let roles = roles(&db).await;
        assert_eq!(
            roles.keys().map(String::as_str).collect::<Vec<_>>(),
            ["Администратор", "ПРИЕМЩИК", "мастер"],
            "{label}"
        );
        assert_eq!(
            roles["мастер"],
            (r#"["orders_create"]"#.to_owned(), 4),
            "{label}"
        );
        assert_eq!(roles["ПРИЕМЩИК"], ("[]".to_owned(), 2), "{label}");
    }
    for (label, db) in all_backends_up_to(BEFORE_BUILTIN_ROLES).await {
        exec(
            &db,
            r#"INSERT INTO roles (name, permissions, version) VALUES ('Мастер', '[]', 1)"#,
        )
        .await;
        mw_db::migrate(&db).await.unwrap();
        let roles = roles(&db).await;
        assert_eq!(roles.len(), 3, "{label}");
        assert_eq!(roles["Мастер"], ("[]".to_owned(), 1), "{label}");
        assert_eq!(
            roles["Приёмщик"],
            (CLERK_PERMISSIONS.to_owned(), 1),
            "{label}"
        );
    }
}

#[tokio::test]
async fn existing_users_keep_sign_in_and_are_not_masters() {
    for (label, db) in all_backends_up_to(BEFORE_USER_FLAGS).await {
        exec(
            &db,
            "INSERT INTO users (login, login_key, display_name, role_id, password_hash, \
             password_scheme, active, created_at, updated_at, version) \
             VALUES ('Ирина', 'ирина', 'Ирина', 1, 'aaaa', 'legacy_sha1', TRUE, \
             '2026-11-01 10:00:00', '2026-11-01 10:00:00', 3)",
        )
        .await;
        mw_db::migrate(&db).await.unwrap();
        let sql = "SELECT login, password_hash, role_id, active, version, can_sign_in, is_master \
                   FROM users";
        let rows = db
            .query_all_raw(Statement::from_string(db.get_database_backend(), sql))
            .await
            .unwrap();
        assert_eq!(rows.len(), 1, "{label}");
        let r = &rows[0];
        let got: (String, String, i64, bool, i64, bool, bool) = (
            r.try_get_by_index(0).unwrap(),
            r.try_get_by_index(1).unwrap(),
            r.try_get_by_index(2).unwrap(),
            r.try_get_by_index(3).unwrap(),
            r.try_get_by_index(4).unwrap(),
            r.try_get_by_index(5).unwrap(),
            r.try_get_by_index(6).unwrap(),
        );
        assert_eq!(
            got,
            ("Ирина".into(), "aaaa".into(), 1, true, 3, true, false),
            "{label}"
        );
    }
}
