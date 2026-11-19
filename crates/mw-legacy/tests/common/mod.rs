//! Общие помощники интеграционных тестов импорта.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use chrono::{TimeZone, Utc};
use mw_core::{Actor, ClientPhone, ClientRef, Core, FixedClock, NewClient, NewOrder};
use mw_db::entity::legacy_row;
use mw_legacy::ImportOptions;
use mw_legacy::fixture::{FixtureOptions, FixtureVersion, write_fixture};
use sea_orm::{ColumnTrait, ConnectionTrait, DatabaseBackend, EntityTrait, QueryFilter, Statement};
use sha2::{Digest, Sha256};
use sqlx::Connection;
use sqlx::sqlite::SqliteConnection;
use tempfile::TempDir;

pub struct TestCore {
    pub label: &'static str,
    pub core: Core,
}

/// По одному `Core` на бэкенд, пояс UTC, часы 2026-11-11T09:00:00Z.
pub async fn cores() -> Vec<TestCore> {
    let mut out = Vec::new();
    for (label, db) in mw_db::testing::all_backends().await {
        let clock = Arc::new(FixedClock::new(
            Utc.with_ymd_and_hms(2026, 11, 11, 9, 0, 0).unwrap(),
        ));
        let core = Core::open(db, clock, "UTC").await.unwrap();
        out.push(TestCore { label, core });
    }
    out
}

pub fn admin() -> Actor {
    Actor::unrestricted("Тест")
}

pub fn sample_new_order() -> NewOrder {
    NewOrder {
        client: ClientRef::New(NewClient {
            full_name: "Пётр Иванов".into(),
            phones: vec![ClientPhone {
                phone: "+7 921 123-45-67".into(),
                note: String::new(),
            }],
            ..Default::default()
        }),
        device_type: "Телефон".into(),
        brand: "Samsung".into(),
        model: "A52".into(),
        malfunction: "Разбит экран".into(),
        warranty: "30 дней".into(),
        ..Default::default()
    }
}

pub async fn fixture_dir(version: FixtureVersion) -> TempDir {
    fixture_dir_with(version, 0).await
}

pub async fn fixture_dir_with(version: FixtureVersion, extra_orders: usize) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    write_fixture(
        dir.path(),
        &FixtureOptions {
            version,
            extra_orders,
        },
    )
    .await
    .unwrap();
    dir
}

/// `attachments` и `backups` — рядом, внутри `data`.
pub fn options(src: &Path, data: &Path, replace: bool) -> ImportOptions {
    ImportOptions {
        source: src.to_path_buf(),
        attachments_dir: data.join("attachments"),
        backups_dir: data.join("backups"),
        replace,
        progress: None,
    }
}

/// Путь (через `/`) → sha256 содержимого для каждого файла дерева.
pub fn hash_tree(path: &Path) -> BTreeMap<String, String> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let relative = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(
                    relative,
                    hex::encode(Sha256::digest(std::fs::read(&path).unwrap())),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(path, path, &mut out);
    out
}

/// JSON всех строк всех таблиц одной строкой.
pub async fn dump_all_tables(core: &Core) -> String {
    let db = core.db();
    let backend = db.get_database_backend();
    let tables_sql = match backend {
        DatabaseBackend::Postgres => {
            "SELECT table_name AS name FROM information_schema.tables \
             WHERE table_schema = current_schema() AND table_type = 'BASE TABLE'"
        }
        _ => "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
    };
    let tables = db
        .query_all_raw(Statement::from_string(backend, tables_sql))
        .await
        .unwrap();
    let mut out = String::new();
    for row in tables {
        let table: String = row.try_get("", "name").unwrap();
        let sql = match backend {
            DatabaseBackend::Postgres => {
                format!("SELECT coalesce(json_agg(t)::text, '[]') AS j FROM \"{table}\" t")
            }
            _ => {
                let columns = db
                    .query_all_raw(Statement::from_string(
                        backend,
                        format!("SELECT name FROM pragma_table_info('{table}')"),
                    ))
                    .await
                    .unwrap();
                let pairs: Vec<String> = columns
                    .iter()
                    .map(|c| {
                        let name: String = c.try_get("", "name").unwrap();
                        format!("'{name}', \"{name}\"")
                    })
                    .collect();
                format!(
                    "SELECT coalesce(json_group_array(json_object({})), '[]') AS j FROM \"{table}\"",
                    pairs.join(", ")
                )
            }
        };
        let json = db
            .query_one_raw(Statement::from_string(backend, sql))
            .await
            .unwrap()
            .unwrap();
        let json: String = json.try_get("", "j").unwrap();
        out.push_str(&format!("{table}: {json}\n"));
    }
    out
}

/// Прямые `COUNT(*)` по исходной базе (0, если таблицы нет).
pub struct SourceCounts {
    pub catalog: u64,
    pub clients_map: u64,
    pub states_map: u64,
    pub history_bd: u64,
}

pub async fn source_counts(path: &Path) -> SourceCounts {
    let file = std::fs::read_dir(path)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("baza.sqlite"))
        })
        .unwrap();
    let mut conn = SqliteConnection::connect(&format!("sqlite://{}?mode=ro", file.display()))
        .await
        .unwrap();
    let mut counts = [0u64; 4];
    for (i, table) in ["Catalog", "ClientsMap", "StatesMap", "HistoryBD"]
        .iter()
        .enumerate()
    {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND lower(name) = lower(?)",
        )
        .bind(table)
        .fetch_one(&mut conn)
        .await
        .unwrap();
        if exists > 0 {
            let n: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                "SELECT COUNT(*) FROM \"{table}\""
            )))
            .fetch_one(&mut conn)
            .await
            .unwrap();
            counts[i] = n as u64;
        }
    }
    conn.close().await.unwrap();
    SourceCounts {
        catalog: counts[0],
        clients_map: counts[1],
        states_map: counts[2],
        history_bd: counts[3],
    }
}

/// Суммы копеек `estimate_from`, `prepayment`, `final_price`, `extra_costs`, `discount` по всем заказам.
pub async fn money_totals(core: &Core) -> [i64; 5] {
    let db = core.db();
    let row = db
        .query_one_raw(Statement::from_string(
            db.get_database_backend(),
            "SELECT CAST(COALESCE(SUM(estimate_from), 0) AS BIGINT) AS a, \
                    CAST(COALESCE(SUM(prepayment), 0) AS BIGINT) AS b, \
                    CAST(COALESCE(SUM(final_price), 0) AS BIGINT) AS c, \
                    CAST(COALESCE(SUM(extra_costs), 0) AS BIGINT) AS d, \
                    CAST(COALESCE(SUM(discount), 0) AS BIGINT) AS e FROM orders",
        ))
        .await
        .unwrap()
        .unwrap();
    ["a", "b", "c", "d", "e"].map(|c| row.try_get::<i64>("", c).unwrap())
}

/// Куда попала исходная строка: (таблица, id).
pub async fn legacy_target(core: &Core, source: &str, key: &str) -> Option<(String, i64)> {
    let row = legacy_row::Entity::find()
        .filter(legacy_row::Column::Source.eq(source))
        .filter(legacy_row::Column::SourceKey.eq(key))
        .one(core.db())
        .await
        .unwrap()?;
    Some((row.target_table?, row.target_id?))
}

/// Файловый SQLite с миграциями.
pub async fn file_sqlite_core(path: &Path) -> Core {
    let db = mw_db::connect(&mw_db::sqlite_url(path)).await.unwrap();
    mw_db::migrate(&db).await.unwrap();
    let clock = Arc::new(FixedClock::new(
        Utc.with_ymd_and_hms(2026, 11, 11, 9, 0, 0).unwrap(),
    ));
    Core::open(db, clock, "UTC").await.unwrap()
}

pub async fn order_count_in_file(path: &Path) -> u64 {
    let mut conn = SqliteConnection::connect(&format!("sqlite://{}?mode=ro", path.display()))
        .await
        .unwrap();
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM orders")
        .fetch_one(&mut conn)
        .await
        .unwrap();
    conn.close().await.unwrap();
    n as u64
}
