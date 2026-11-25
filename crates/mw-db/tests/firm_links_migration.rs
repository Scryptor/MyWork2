//! Три ссылки фирмы (`firm_telegram`, `firm_google_maps`, `firm_yandex_maps`) переезжают в список
//! `firm_links`; пустые старые настройки оставляют умолчания.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mw_db::testing::all_backends_up_to;
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use serde_json::{Value, json};

/// Миграций до переноса ссылок.
const BEFORE_FIRM_LINKS: u32 = 16;

async fn exec(db: &DatabaseConnection, sql: &str) {
    db.execute_raw(Statement::from_string(db.get_database_backend(), sql))
        .await
        .unwrap_or_else(|e| panic!("{sql}: {e}"));
}

async fn seed(db: &DatabaseConnection, key: &str, value: &str) {
    exec(
        db,
        &format!(
            "INSERT INTO settings (key, value) VALUES ('{key}', '{}')",
            json!(value)
        ),
    )
    .await;
}

async fn settings_keys(db: &DatabaseConnection) -> Vec<String> {
    let rows = db
        .query_all_raw(Statement::from_string(
            db.get_database_backend(),
            "SELECT key FROM settings ORDER BY key",
        ))
        .await
        .unwrap();
    rows.iter()
        .map(|r| r.try_get_by_index(0).unwrap())
        .collect()
}

async fn links(db: &DatabaseConnection) -> Option<Value> {
    let rows = db
        .query_all_raw(Statement::from_string(
            db.get_database_backend(),
            "SELECT value FROM settings WHERE key = 'firm_links'",
        ))
        .await
        .unwrap();
    rows.first().map(|r| {
        let v: String = r.try_get_by_index(0).unwrap();
        serde_json::from_str(&v).unwrap()
    })
}

fn link(id: &str, name: &str, url: &str) -> Value {
    json!({"id": id, "name": name, "url": url, "enabled": true})
}

#[tokio::test]
async fn old_links_move_into_the_list() {
    for (label, db) in all_backends_up_to(BEFORE_FIRM_LINKS).await {
        seed(&db, "firm_telegram", "https://t.me/example_service").await;
        seed(
            &db,
            "firm_yandex_maps",
            "https://yandex.ru/maps/?text=example",
        )
        .await;
        seed(
            &db,
            "firm_google_maps",
            "https://maps.google.com/?q=example",
        )
        .await;
        mw_db::migrate(&db).await.unwrap();
        assert_eq!(
            links(&db).await,
            Some(json!([
                link(
                    "yandex_maps",
                    "Яндекс Карты",
                    "https://yandex.ru/maps/?text=example"
                ),
                link("2gis", "2ГИС", ""),
                link("telegram", "Telegram", "https://t.me/example_service"),
                link("max", "MAX", ""),
                link(
                    "google_maps",
                    "Google Карты",
                    "https://maps.google.com/?q=example"
                ),
            ])),
            "{label}"
        );
        let keys = settings_keys(&db).await;
        for old in ["firm_telegram", "firm_google_maps", "firm_yandex_maps"] {
            assert!(!keys.iter().any(|k| k == old), "{label}: {old} остался");
        }
    }
}

#[tokio::test]
async fn only_telegram_set_leaves_google_out_of_the_list() {
    for (label, db) in all_backends_up_to(BEFORE_FIRM_LINKS).await {
        seed(&db, "firm_telegram", "https://t.me/example_service").await;
        seed(&db, "firm_google_maps", "").await;
        mw_db::migrate(&db).await.unwrap();
        let list = links(&db).await.unwrap();
        let ids: Vec<_> = list
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l["id"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(ids, ["yandex_maps", "2gis", "telegram", "max"], "{label}");
        assert_eq!(list[2]["url"], "https://t.me/example_service", "{label}");
    }
}

#[tokio::test]
async fn empty_old_links_leave_the_defaults() {
    for (label, db) in all_backends_up_to(BEFORE_FIRM_LINKS).await {
        seed(&db, "firm_telegram", "").await;
        seed(&db, "firm_google_maps", "").await;
        seed(&db, "firm_yandex_maps", "").await;
        mw_db::migrate(&db).await.unwrap();
        assert_eq!(links(&db).await, None, "{label}");
        let keys = settings_keys(&db).await;
        assert!(
            !keys
                .iter()
                .any(|k| k.starts_with("firm_") && k.ends_with("_maps"))
        );
        assert!(!keys.iter().any(|k| k == "firm_telegram"), "{label}");
    }
}

#[tokio::test]
async fn running_the_migration_again_changes_nothing() {
    for (label, db) in all_backends_up_to(BEFORE_FIRM_LINKS).await {
        seed(&db, "firm_telegram", "https://t.me/example_service").await;
        mw_db::migrate(&db).await.unwrap();
        let first = links(&db).await;
        assert!(first.is_some(), "{label}");
        // Свои правки списка и вернувшийся старый ключ: повторный запуск ничего не перезаписывает.
        let edited = json!([
            link("yandex_maps", "Яндекс Карты", ""),
            link("2gis", "2ГИС", ""),
            link("telegram", "Telegram", "https://t.me/changed"),
            link("max", "MAX", "")
        ]);
        exec(
            &db,
            &format!("UPDATE settings SET value = '{edited}' WHERE key = 'firm_links'"),
        )
        .await;
        seed(&db, "firm_telegram", "https://t.me/example_service").await;
        // Забываем запись о миграции, чтобы `up` выполнился ещё раз.
        exec(
            &db,
            "DELETE FROM seaql_migrations WHERE version = 'm20261125_000001_firm_links'",
        )
        .await;
        mw_db::migrate(&db).await.unwrap();
        assert_eq!(links(&db).await, Some(edited), "{label}");
        assert!(
            !settings_keys(&db)
                .await
                .iter()
                .any(|k| k == "firm_telegram"),
            "{label}"
        );
    }
}
