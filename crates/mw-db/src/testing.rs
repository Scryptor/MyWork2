//! Помощники для тестов с БД. Собираются всегда, но используются только тестами.

use std::time::Duration;

use sea_orm::sqlx::sqlite::SqliteJournalMode;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};

use crate::connect::sqlite_pragmas;
use crate::{migrate, migrate_up_to};

/// База SQLite в памяти с миграциями. Пул ровно из одного соединения: у каждого соединения
/// своя база в памяти, и оно не должно закрываться по таймауту.
pub async fn sqlite_memory() -> DatabaseConnection {
    sqlite_memory_steps(None).await
}

async fn sqlite_memory_steps(steps: Option<u32>) -> DatabaseConnection {
    let mut options = ConnectOptions::new("sqlite::memory:");
    options
        .max_connections(1)
        .min_connections(1)
        .idle_timeout(None::<Duration>)
        .max_lifetime(None::<Duration>)
        .sqlx_logging(false)
        .map_sqlx_sqlite_opts(|o| sqlite_pragmas(o, SqliteJournalMode::Memory));
    let db = Database::connect(options)
        .await
        .unwrap_or_else(|e| panic!("не удалось открыть SQLite в памяти: {e}"));
    apply(&db, steps)
        .await
        .unwrap_or_else(|e| panic!("миграция SQLite не применилась: {e}"));
    db
}

/// Изолированная схема `mw_t_<16 hex>` в PostgreSQL из `MW_TEST_PG_URL`, с миграциями.
/// `None`, если переменная не задана; если задана, но подключиться нельзя — паника.
pub async fn postgres_isolated() -> Option<DatabaseConnection> {
    postgres_isolated_steps(None).await
}

async fn postgres_isolated_steps(steps: Option<u32>) -> Option<DatabaseConnection> {
    let url = std::env::var("MW_TEST_PG_URL").ok()?;
    let mut random = [0u8; 8];
    getrandom::fill(&mut random).unwrap_or_else(|e| panic!("нет случайных байт: {e}"));
    let schema = format!("mw_t_{}", hex::encode(random));

    let admin = Database::connect(url.as_str())
        .await
        .unwrap_or_else(|e| panic!("MW_TEST_PG_URL задана, но подключиться не удалось: {e}"));
    admin
        .execute_unprepared(&format!("CREATE SCHEMA {schema}"))
        .await
        .unwrap_or_else(|e| panic!("не удалось создать схему {schema}: {e}"));
    admin.close().await.ok();

    let mut options = ConnectOptions::new(url);
    options
        .max_connections(4)
        .sqlx_logging(false)
        .set_schema_search_path(schema.clone());
    let db = Database::connect(options)
        .await
        .unwrap_or_else(|e| panic!("не удалось подключиться к схеме {schema}: {e}"));
    apply(&db, steps)
        .await
        .unwrap_or_else(|e| panic!("миграция PostgreSQL не применилась: {e}"));
    Some(db)
}

async fn apply(db: &DatabaseConnection, steps: Option<u32>) -> Result<(), sea_orm::DbErr> {
    match steps {
        Some(n) => migrate_up_to(db, n).await,
        None => migrate(db).await,
    }
}

/// Все доступные бэкенды: SQLite всегда, PostgreSQL — при заданной `MW_TEST_PG_URL`.
pub async fn all_backends() -> Vec<(&'static str, DatabaseConnection)> {
    all_backends_steps(None).await
}

/// То же, что [`all_backends`], но применены только первые `steps` миграций.
pub async fn all_backends_up_to(steps: u32) -> Vec<(&'static str, DatabaseConnection)> {
    all_backends_steps(Some(steps)).await
}

async fn all_backends_steps(steps: Option<u32>) -> Vec<(&'static str, DatabaseConnection)> {
    let mut backends = vec![("sqlite", sqlite_memory_steps(steps).await)];
    if let Some(db) = postgres_isolated_steps(steps).await {
        backends.push(("postgres", db));
    }
    backends
}
