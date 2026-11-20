//! Миграция данных «мастера → пользователи» на базе, заполненной до обновления: имена мастеров из
//! заказов, справочника «Мастера» и «мастера по умолчанию» становятся пользователями-мастерами.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use chrono::{TimeZone, Utc};
use mw_core::{Core, FixedClock};
use mw_db::testing::all_backends_up_to;
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};

/// Миграций до столбца `orders.master_user_id` и переноса мастеров.
const BEFORE_MASTERS: u32 = 9;
const MIGRATION: &str = "m20261120_000004_masters_to_users";
const AT: &str = "'2026-11-01 10:00:00'";

async fn exec(db: &DatabaseConnection, sql: &str) {
    db.execute_raw(Statement::from_string(db.get_database_backend(), sql))
        .await
        .unwrap_or_else(|e| panic!("{sql}: {e}"));
}

async fn rows(db: &DatabaseConnection, sql: &str) -> Vec<sea_orm::QueryResult> {
    db.query_all_raw(Statement::from_string(db.get_database_backend(), sql))
        .await
        .unwrap_or_else(|e| panic!("{sql}: {e}"))
}

async fn one_id(db: &DatabaseConnection, sql: &str) -> i64 {
    rows(db, sql).await[0].try_get_by_index(0).unwrap()
}

fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

/// Заказ со своим текстом мастера и версией 3 (у клиента №1), как в базе до обновления.
async fn insert_order(db: &DatabaseConnection, master: &str, deleted: bool) -> i64 {
    let deleted_at = if deleted { AT } else { "NULL" };
    one_id(
        db,
        &format!(
            "INSERT INTO orders (client_id, status_id, received_at, created_at, updated_at, \
             deleted_at, master, version) \
             VALUES (1, (SELECT MIN(id) FROM statuses), {AT}, {AT}, {AT}, {deleted_at}, {}, 3) \
             RETURNING id",
            quote(master)
        ),
    )
    .await
}

async fn insert_dictionary(db: &DatabaseConnection, value: &str, archived: bool) {
    exec(
        db,
        &format!(
            "INSERT INTO dictionary_items (kind, value, sort_order, archived) \
             VALUES ('master', {}, 0, {})",
            quote(value),
            if archived { "TRUE" } else { "FALSE" }
        ),
    )
    .await;
}

/// Пользователь со входом и паролем, как после импорта MyWork 2; версия 3.
async fn insert_user(db: &DatabaseConnection, login: &str, name: &str, active: bool) -> i64 {
    one_id(
        db,
        &format!(
            "INSERT INTO users (login, login_key, display_name, role_id, password_hash, \
             password_scheme, active, created_at, updated_at, version, can_sign_in, is_master) \
             VALUES ({}, {}, {}, (SELECT id FROM roles WHERE name = 'Администратор'), \
             'aaaa', 'legacy_sha1', {}, {AT}, {AT}, 3, TRUE, FALSE) RETURNING id",
            quote(login),
            quote(&mw_domain::search::normalize_search(login)),
            quote(name),
            if active { "TRUE" } else { "FALSE" }
        ),
    )
    .await
}

async fn prepare(db: &DatabaseConnection) {
    exec(
        db,
        &format!("INSERT INTO clients (id, full_name, created_at, updated_at) VALUES (1, 'Клиент', {AT}, {AT})"),
    )
    .await;
}

async fn open(db: DatabaseConnection) -> Core {
    let clock = Arc::new(FixedClock::new(
        Utc.with_ymd_and_hms(2026, 11, 21, 9, 0, 0).unwrap(),
    ));
    Core::open(db, clock, "UTC").await.unwrap()
}

/// (номер, логин, имя, роль, хэш, может входить, мастер, активен, версия, ключ логина).
type UserRow = (
    i64,
    String,
    String,
    String,
    String,
    bool,
    bool,
    bool,
    i64,
    String,
);

async fn users(db: &DatabaseConnection) -> Vec<UserRow> {
    rows(
        db,
        "SELECT u.id, u.login, u.display_name, r.name, u.password_hash, u.can_sign_in, \
         u.is_master, u.active, u.version, u.login_key \
         FROM users u JOIN roles r ON r.id = u.role_id ORDER BY u.id",
    )
    .await
    .iter()
    .map(|r| {
        (
            r.try_get_by_index(0).unwrap(),
            r.try_get_by_index(1).unwrap(),
            r.try_get_by_index(2).unwrap(),
            r.try_get_by_index(3).unwrap(),
            r.try_get_by_index(4).unwrap(),
            r.try_get_by_index(5).unwrap(),
            r.try_get_by_index(6).unwrap(),
            r.try_get_by_index(7).unwrap(),
            r.try_get_by_index(8).unwrap(),
            r.try_get_by_index(9).unwrap(),
        )
    })
    .collect()
}

fn user_named<'a>(all: &'a [UserRow], name: &str) -> &'a UserRow {
    all.iter()
        .find(|u| u.2 == name)
        .unwrap_or_else(|| panic!("нет пользователя «{name}»: {all:?}"))
}

/// (номер, мастер текстом, номер мастера, версия, когда изменён).
async fn orders(db: &DatabaseConnection) -> Vec<(i64, String, Option<i64>, i64, String)> {
    rows(
        db,
        "SELECT id, master, master_user_id, version, CAST(updated_at AS TEXT) FROM orders ORDER BY id",
    )
    .await
    .iter()
    .map(|r| {
        (
            r.try_get_by_index(0).unwrap(),
            r.try_get_by_index(1).unwrap(),
            r.try_get_by_index(2).unwrap(),
            r.try_get_by_index(3).unwrap(),
            r.try_get_by_index(4).unwrap(),
        )
    })
    .collect()
}

async fn dictionary(db: &DatabaseConnection) -> Vec<(String, bool)> {
    rows(
        db,
        "SELECT value, archived FROM dictionary_items WHERE kind = 'master' ORDER BY id",
    )
    .await
    .iter()
    .map(|r| {
        (
            r.try_get_by_index(0).unwrap(),
            r.try_get_by_index(1).unwrap(),
        )
    })
    .collect()
}

async fn summaries(db: &DatabaseConnection) -> Vec<(String, String)> {
    rows(
        db,
        "SELECT summary, user_name FROM audit_log WHERE action = 'masters.migrated' ORDER BY id",
    )
    .await
    .iter()
    .map(|r| {
        (
            r.try_get_by_index(0).unwrap(),
            r.try_get_by_index(1).unwrap(),
        )
    })
    .collect()
}

async fn settings(db: &DatabaseConnection) -> Vec<(String, String)> {
    rows(db, "SELECT key, value FROM settings ORDER BY key")
        .await
        .iter()
        .map(|r| {
            (
                r.try_get_by_index(0).unwrap(),
                r.try_get_by_index(1).unwrap(),
            )
        })
        .collect()
}

#[tokio::test]
async fn every_master_name_becomes_one_master_user() {
    for (label, db) in all_backends_up_to(BEFORE_MASTERS).await {
        prepare(&db).await;
        let a = insert_order(&db, "Иванов", false).await;
        let b = insert_order(&db, " иванов ", false).await;
        let c = insert_order(&db, "ИВАНОВ", false).await;
        let gone = insert_order(&db, "Петров", true).await;
        let none = insert_order(&db, "", false).await;
        insert_dictionary(&db, "Иванов И.", false).await;
        insert_dictionary(&db, "Иванов", true).await;
        insert_dictionary(&db, "Сидоров", false).await;
        exec(
            &db,
            r#"INSERT INTO settings (key, value) VALUES ('default_master', '"сидоров"')"#,
        )
        .await;
        mw_db::migrate(&db).await.unwrap();

        let all = users(&db).await;
        assert_eq!(all.len(), 4, "{label}: {all:?}");
        // Два написания одного имени — один пользователь; написание из справочника.
        let ivanov = user_named(&all, "Иванов");
        let petrov = user_named(&all, "Петров");
        let sidorov = user_named(&all, "Сидоров");
        let ivanov_i = user_named(&all, "Иванов И.");
        for u in [ivanov, petrov, sidorov, ivanov_i] {
            assert_eq!(
                (u.1.as_str(), u.3.as_str(), u.4.as_str(), u.5, u.6, u.7, u.8),
                ("", "Мастер", "", false, true, true, 1),
                "{label}: {u:?}"
            );
            assert!(u.9.starts_with("NO-LOGIN:"), "{label}: {u:?}");
        }
        assert_eq!(
            orders(&db).await,
            [
                (
                    a,
                    "Иванов".into(),
                    Some(ivanov.0),
                    3,
                    "2026-11-01 10:00:00".into()
                ),
                (
                    b,
                    " иванов ".into(),
                    Some(ivanov.0),
                    3,
                    "2026-11-01 10:00:00".into()
                ),
                (
                    c,
                    "ИВАНОВ".into(),
                    Some(ivanov.0),
                    3,
                    "2026-11-01 10:00:00".into()
                ),
                (
                    gone,
                    "Петров".into(),
                    Some(petrov.0),
                    3,
                    "2026-11-01 10:00:00".into()
                ),
                (none, String::new(), None, 3, "2026-11-01 10:00:00".into()),
            ],
            "{label}"
        );
        assert_eq!(
            dictionary(&db).await,
            [
                ("Иванов И.".into(), true),
                ("Иванов".into(), true),
                ("Сидоров".into(), true)
            ],
            "{label}: значения остаются, но в архиве"
        );
        assert_eq!(
            summaries(&db).await,
            [(
                "Мастера перенесены в пользователей: создано 4, отмечено 0".into(),
                "Обновление программы".into()
            )],
            "{label}"
        );

        let core = open(db).await;
        assert!(
            core.is_open_mode().await.unwrap(),
            "{label}: режим без входа"
        );
        assert_eq!(
            core.settings().default_master_user_id,
            Some(sidorov.0),
            "{label}"
        );
        let masters = core
            .list_masters(&mw_core::Actor::unrestricted("Тест"))
            .await
            .unwrap();
        assert_eq!(masters.len(), 4, "{label}");
    }
}

#[tokio::test]
async fn a_user_with_the_same_name_is_marked_not_doubled() {
    for (label, db) in all_backends_up_to(BEFORE_MASTERS).await {
        prepare(&db).await;
        let irina = insert_user(&db, "Ирина", "Ирина", true).await;
        let petr = insert_user(&db, "petr", "Пётр Петров", true).await;
        let oleg = insert_user(&db, "oleg", "Олег", false).await;
        let kolya = insert_user(&db, "kolya", "", true).await;
        let a = insert_order(&db, "ирина", false).await;
        let b = insert_order(&db, "ПЕТР ПЕТРОВ", false).await;
        let c = insert_order(&db, "олег", false).await;
        let d = insert_order(&db, "KOLYA", false).await;
        let e = insert_order(&db, "Новый", false).await;
        let before = users(&db).await;
        mw_db::migrate(&db).await.unwrap();

        let all = users(&db).await;
        assert_eq!(all.len(), 5, "{label}: {all:?}");
        for (old, new) in before.iter().zip(&all) {
            // Отмечен «Мастер»; логин, имя, роль, пароль, вход и активность — прежние.
            assert_eq!(
                (&old.0, &old.1, &old.2, &old.3, &old.4, old.5, old.7, &old.9),
                (&new.0, &new.1, &new.2, &new.3, &new.4, new.5, new.7, &new.9),
                "{label}"
            );
            assert_eq!((new.6, new.8), (true, 4), "{label}: {new:?}");
        }
        let created = user_named(&all, "Новый");
        assert_eq!((created.5, created.6), (false, true), "{label}");
        let linked: Vec<(i64, Option<i64>)> =
            orders(&db).await.into_iter().map(|o| (o.0, o.2)).collect();
        assert_eq!(
            linked,
            [
                (a, Some(irina)),
                (b, Some(petr)),
                (c, Some(oleg)),
                (d, Some(kolya)),
                (e, Some(created.0))
            ],
            "{label}"
        );
        assert_eq!(
            summaries(&db).await[0].0,
            "Мастера перенесены в пользователей: создано 1, отмечено 4",
            "{label}"
        );
        let core = open(db).await;
        assert!(!core.is_open_mode().await.unwrap(), "{label}");
        let masters = core
            .list_masters(&mw_core::Actor::unrestricted("Тест"))
            .await
            .unwrap();
        // Отключённый Олег — последним.
        assert_eq!(masters.last().map(|m| m.id), Some(oleg), "{label}");
    }
}

#[tokio::test]
async fn running_the_migration_again_changes_nothing() {
    for (label, db) in all_backends_up_to(BEFORE_MASTERS).await {
        prepare(&db).await;
        insert_user(&db, "Ирина", "Ирина", true).await;
        insert_order(&db, "ирина", false).await;
        insert_order(&db, "Иванов", false).await;
        insert_order(&db, "", false).await;
        insert_dictionary(&db, "Сидоров", false).await;
        exec(
            &db,
            r#"INSERT INTO settings (key, value) VALUES ('default_master', '"Иванов"')"#,
        )
        .await;
        mw_db::migrate(&db).await.unwrap();
        let snapshot = (
            users(&db).await,
            orders(&db).await,
            dictionary(&db).await,
            settings(&db).await,
            summaries(&db).await,
        );
        assert_eq!(snapshot.0.len(), 3, "{label}");
        exec(
            &db,
            &format!("DELETE FROM seaql_migrations WHERE version = '{MIGRATION}'"),
        )
        .await;
        mw_db::migrate(&db).await.unwrap();
        let again = (
            users(&db).await,
            orders(&db).await,
            dictionary(&db).await,
            settings(&db).await,
            summaries(&db).await,
        );
        assert_eq!(again, snapshot, "{label}");
    }
}

#[tokio::test]
async fn a_missing_master_role_is_added_and_an_existing_one_is_kept() {
    for (label, db) in all_backends_up_to(BEFORE_MASTERS).await {
        prepare(&db).await;
        exec(&db, "DELETE FROM roles WHERE name = 'Мастер'").await;
        insert_order(&db, "Иванов", false).await;
        mw_db::migrate(&db).await.unwrap();
        let roles: Vec<(String, String)> = rows(
            &db,
            "SELECT name, permissions FROM roles WHERE name = 'Мастер'",
        )
        .await
        .iter()
        .map(|r| {
            (
                r.try_get_by_index(0).unwrap(),
                r.try_get_by_index(1).unwrap(),
            )
        })
        .collect();
        assert_eq!(
            roles,
            [("Мастер".into(), r#"["orders_edit","clients_view"]"#.into())],
            "{label}"
        );
        assert_eq!(user_named(&users(&db).await, "Иванов").3, "Мастер");
    }
    for (label, db) in all_backends_up_to(BEFORE_MASTERS).await {
        prepare(&db).await;
        exec(
            &db,
            r#"UPDATE roles SET name = 'МАСТЕР', permissions = '["orders_create"]' WHERE name = 'Мастер'"#,
        )
        .await;
        insert_order(&db, "Иванов", false).await;
        mw_db::migrate(&db).await.unwrap();
        let names: Vec<String> = rows(&db, "SELECT name FROM roles ORDER BY id")
            .await
            .iter()
            .map(|r| r.try_get_by_index(0).unwrap())
            .collect();
        assert_eq!(names, ["Администратор", "МАСТЕР", "Приёмщик"], "{label}");
        assert_eq!(user_named(&users(&db).await, "Иванов").3, "МАСТЕР");
    }
}

#[tokio::test]
async fn a_base_without_masters_gets_no_entry() {
    for (label, db) in all_backends_up_to(BEFORE_MASTERS).await {
        prepare(&db).await;
        insert_order(&db, "  ", false).await;
        mw_db::migrate(&db).await.unwrap();
        assert!(users(&db).await.is_empty(), "{label}");
        assert!(summaries(&db).await.is_empty(), "{label}");
        let core = open(db).await;
        assert!(core.is_open_mode().await.unwrap());
        assert_eq!(core.settings().default_master_user_id, None);
    }
}
