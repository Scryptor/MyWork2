//! Миграция вложений на уже заполненной базе: файлы старой программы становятся вложениями.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mw_db::testing::all_backends_up_to;
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};

async fn exec(db: &DatabaseConnection, sql: &str) {
    db.execute_raw(Statement::from_string(db.get_database_backend(), sql))
        .await
        .unwrap();
}

async fn rows(db: &DatabaseConnection) -> Vec<(i64, String, String, String, i64, bool, String)> {
    let sql = "SELECT order_id, kind, name, content_type, size, legacy, CAST(created_at AS TEXT) \
               FROM attachments ORDER BY path";
    db.query_all_raw(Statement::from_string(db.get_database_backend(), sql))
        .await
        .unwrap()
        .into_iter()
        .map(|r| {
            (
                r.try_get_by_index(0).unwrap(),
                r.try_get_by_index(1).unwrap(),
                r.try_get_by_index(2).unwrap(),
                r.try_get_by_index(3).unwrap(),
                r.try_get_by_index(4).unwrap(),
                r.try_get_by_index(5).unwrap(),
                r.try_get_by_index(6).unwrap(),
            )
        })
        .collect()
}

#[tokio::test]
async fn migration_turns_existing_client_files_into_attachments() {
    for (label, db) in all_backends_up_to(5).await {
        exec(
            &db,
            "INSERT INTO legacy_imports (id, imported_at, source_path, mode, report) \
             VALUES (1, '2026-11-01 10:00:00', 'x', 'into_empty', '{}')",
        )
        .await;
        let row = |key: &str, table: &str, raw: &str| {
            format!(
                "INSERT INTO legacy_rows (import_id, source, source_key, target_table, target_id, raw) \
                 VALUES (1, 'ClientFiles', '{key}', '{table}', 5, '{raw}')"
            )
        };
        for sql in [
            row(
                "a",
                "orders",
                r#"{"stored_as":"attachments/orders/5/photo.JPG","size":12,"sha256":"00"}"#,
            ),
            row(
                "b",
                "orders",
                r#"{"stored_as":"attachments/orders/5/video.mp4","size":7}"#,
            ),
            row("c", "orders", r#"{"other":true}"#),
            row(
                "d",
                "clients",
                r#"{"stored_as":"attachments/orders/5/skip.txt","size":1}"#,
            ),
        ] {
            exec(&db, &sql).await;
        }

        mw_db::migrate(&db).await.unwrap();

        let got = rows(&db).await;
        assert_eq!(got.len(), 2, "{label}");
        assert_eq!(
            (
                got[0].0,
                got[0].1.as_str(),
                got[0].2.as_str(),
                got[0].4,
                got[0].5
            ),
            (5, "photo", "photo.JPG", 12, true),
            "{label}"
        );
        assert_eq!(got[0].3, "image/jpeg", "{label}");
        assert_eq!(
            (got[1].1.as_str(), got[1].3.as_str()),
            ("file", "video/mp4"),
            "{label}"
        );
        assert!(
            got[1].6.starts_with("2026-11-01 10:00:00"),
            "{label}: {}",
            got[1].6
        );

        // Повторный запуск ничего не дублирует.
        assert_eq!(
            mw_db::backfill::legacy_client_files(&db).await.unwrap(),
            0,
            "{label}"
        );
        assert_eq!(rows(&db).await.len(), 2, "{label}");
    }
}
