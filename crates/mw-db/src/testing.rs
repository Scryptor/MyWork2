//! Помощники для тестов с БД. Собираются всегда, но используются только тестами.

use std::time::Duration;

use sea_orm::sqlx::sqlite::SqliteJournalMode;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};

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

/// Комментарий схем, которые создаёт [`postgres_isolated`]: уборка трогает только их.
const SCHEMA_MARK: &str = "mw_db::testing";

/// Изолированная схема `mw_t_<16 hex>` в PostgreSQL из `MW_TEST_PG_URL`, с миграциями.
/// `None`, если переменная не задана; если задана, но подключиться нельзя — паника.
///
/// Схема занята, пока жив её пул: он всегда держит соединение, а каждое соединение называет
/// себя именем схемы (`application_name`). Перед созданием схемы удаляются помеченные схемы
/// без единого сеанса — их тест закончился или процесс упал. Поэтому `MW_TEST_PG_URL` должна
/// вести прямо в PostgreSQL, а не через пулер соединений.
pub async fn postgres_isolated() -> Option<DatabaseConnection> {
    postgres_isolated_steps(None).await
}

async fn postgres_isolated_steps(steps: Option<u32>) -> Option<DatabaseConnection> {
    let url = std::env::var("MW_TEST_PG_URL").ok()?;
    let mut random = [0u8; 8];
    getrandom::fill(&mut random).unwrap_or_else(|e| panic!("нет случайных байт: {e}"));
    let schema = format!("mw_t_{}", hex::encode(random));

    // Пул подключается раньше, чем появляется схема: у неё не бывает момента без сеанса.
    let mut options = ConnectOptions::new(url.as_str());
    options
        .max_connections(4)
        .min_connections(1)
        .idle_timeout(None::<Duration>)
        .max_lifetime(None::<Duration>)
        .sqlx_logging(false)
        .set_application_name(schema.clone())
        .set_schema_search_path(schema.clone());
    let db = Database::connect(options)
        .await
        .unwrap_or_else(|e| panic!("MW_TEST_PG_URL задана, но подключиться не удалось: {e}"));
    drop_abandoned_schemas(&url).await;
    // Две команды одной строкой выполняются одной транзакцией: схемы без пометки не бывает.
    db.execute_unprepared(&format!(
        "CREATE SCHEMA {schema}; COMMENT ON SCHEMA {schema} IS '{SCHEMA_MARK}'"
    ))
    .await
    .unwrap_or_else(|e| panic!("не удалось создать схему {schema}: {e}"));
    apply(&db, steps)
        .await
        .unwrap_or_else(|e| panic!("миграция PostgreSQL не применилась: {e}"));
    Some(db)
}

/// Удаляет помеченные схемы без единого сеанса. Уборка — по возможности: её ошибки тест
/// не роняют, а схему, которую держит чужая транзакция, удалит следующий вызов.
async fn drop_abandoned_schemas(url: &str) {
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .sqlx_logging(false)
        .map_sqlx_postgres_opts(|o| o.options([("lock_timeout", "1s")]));
    let Ok(db) = Database::connect(options).await else {
        return;
    };
    // В случайном порядке: параллельные тесты удаляют разные схемы, а не ждут друг друга.
    let abandoned = db
        .query_all_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT n.nspname FROM pg_namespace n \
             WHERE n.nspname ~ '^mw_t_[0-9a-f]{16}$' \
               AND obj_description(n.oid, 'pg_namespace') = $1 \
               AND NOT EXISTS (SELECT 1 FROM pg_stat_activity a \
                               WHERE a.application_name = n.nspname) \
             ORDER BY random()",
            [SCHEMA_MARK.into()],
        ))
        .await
        .unwrap_or_default();
    for row in abandoned {
        if let Ok(name) = row.try_get_by_index::<String>(0) {
            db.execute_unprepared(&format!("DROP SCHEMA IF EXISTS \"{name}\" CASCADE"))
                .await
                .ok();
        }
    }
    db.close().await.ok();
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
