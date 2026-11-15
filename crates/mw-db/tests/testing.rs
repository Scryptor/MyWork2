//! Уборка тестовых схем PostgreSQL в `mw_db::testing`. Без `MW_TEST_PG_URL` проверять нечего.
// Помощники интеграционных тестов вне `#[test]`-функций: unwrap здесь допустим.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::{Duration, Instant};

use mw_db::testing::postgres_isolated;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};

/// Отдельное подключение к той же базе: смотреть на схемы со стороны.
async fn observer() -> Option<DatabaseConnection> {
    let url = std::env::var("MW_TEST_PG_URL").ok()?;
    Some(Database::connect(url).await.unwrap())
}

async fn holds(db: &DatabaseConnection, sql: &str, name: &str) -> bool {
    db.query_one_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        sql,
        [name.into()],
    ))
    .await
    .unwrap()
    .unwrap()
    .try_get_by_index::<bool>(0)
    .unwrap()
}

async fn schema_exists(db: &DatabaseConnection, name: &str) -> bool {
    holds(
        db,
        "SELECT EXISTS (SELECT 1 FROM pg_namespace WHERE nspname = $1)",
        name,
    )
    .await
}

async fn current_schema(db: &DatabaseConnection) -> String {
    db.query_one_raw(Statement::from_string(
        DbBackend::Postgres,
        "SELECT current_schema()",
    ))
    .await
    .unwrap()
    .unwrap()
    .try_get_by_index::<String>(0)
    .unwrap()
}

/// Ждёт, пока PostgreSQL завершит сеансы закрытого пула: схема занята, пока есть сеанс
/// с `application_name`, равным её имени.
async fn wait_until_sessions_end(db: &DatabaseConnection, schema: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while holds(
        db,
        "SELECT EXISTS (SELECT 1 FROM pg_stat_activity WHERE application_name = $1)",
        schema,
    )
    .await
    {
        assert!(
            Instant::now() < deadline,
            "сеансы схемы {schema} не завершились"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn abandoned_schema_is_dropped_when_next_one_is_created() {
    let Some(observer) = observer().await else {
        return;
    };
    let first = postgres_isolated().await.unwrap();
    let schema = current_schema(&first).await;
    first.close().await.unwrap();
    wait_until_sessions_end(&observer, &schema).await;

    let _next = postgres_isolated().await.unwrap();
    assert!(
        !schema_exists(&observer, &schema).await,
        "брошенная схема {schema} осталась"
    );
}

#[tokio::test]
async fn schema_with_live_pool_survives_next_creation() {
    let Some(observer) = observer().await else {
        return;
    };
    let first = postgres_isolated().await.unwrap();
    let schema = current_schema(&first).await;

    let _next = postgres_isolated().await.unwrap();
    assert!(
        schema_exists(&observer, &schema).await,
        "схему {schema} удалили, хотя её пул жив"
    );
    first
        .execute_unprepared("SELECT count(*) FROM seaql_migrations")
        .await
        .unwrap();
}

#[tokio::test]
async fn unmarked_schema_is_kept() {
    let Some(observer) = observer().await else {
        return;
    };
    // Так выглядят схемы из кода до уборки: ими может пользоваться тест старой ветки.
    let mut random = [0u8; 8];
    getrandom::fill(&mut random).unwrap();
    let schema = format!("mw_t_{}", hex::encode(random));
    observer
        .execute_unprepared(&format!("CREATE SCHEMA {schema}"))
        .await
        .unwrap();

    let _next = postgres_isolated().await.unwrap();
    let kept = schema_exists(&observer, &schema).await;
    observer
        .execute_unprepared(&format!("DROP SCHEMA IF EXISTS {schema}"))
        .await
        .unwrap();
    assert!(kept, "схему {schema} без пометки удалили");
}
