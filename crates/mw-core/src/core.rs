use std::sync::{Arc, RwLock};

use chrono::{NaiveDateTime, SubsecRound, TimeZone};
use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction, IsolationLevel,
    TransactionOptions, TransactionTrait,
};

use crate::auth::{HashGate, MAX_CONCURRENT_PASSWORD_HASHES};
use crate::settings::{self, AppSettings};
use crate::{Clock, CoreError};

/// Приложение: БД, часы и кэш настроек. Дёшево клонируется.
#[derive(Clone)]
pub struct Core {
    pub(crate) db: DatabaseConnection,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) settings: Arc<RwLock<AppSettings>>,
    pub(crate) hash_gate: HashGate,
}

impl Core {
    /// Если настройки `timezone` ещё нет, сохраняет `system_timezone` (неверное имя → `"UTC"`).
    pub async fn open(
        db: DatabaseConnection,
        clock: Arc<dyn Clock>,
        system_timezone: &str,
    ) -> Result<Core, CoreError> {
        let loaded = settings::ensure_timezone(&db, system_timezone).await?;
        crate::search_index::ensure_index(&db).await?;
        Ok(Core {
            db,
            clock,
            settings: Arc::new(RwLock::new(loaded)),
            hash_gate: HashGate::new(MAX_CONCURRENT_PASSWORD_HASHES),
        })
    }

    #[cfg(any(test, feature = "testing"))]
    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    /// Местное время предприятия, до секунд. Единственный источник «сейчас» в приложении.
    pub fn now(&self) -> NaiveDateTime {
        let tz = settings::parse_timezone(&self.settings().timezone);
        tz.from_utc_datetime(&self.clock.now_utc().naive_utc())
            .naive_local()
            .trunc_subsecs(0)
    }

    /// Показания часов `Core` в UTC, без перевода в пояс предприятия (интервалы, ограничение попыток).
    pub fn now_utc(&self) -> chrono::DateTime<chrono::Utc> {
        self.clock.now_utc()
    }

    /// Копия настроек из кэша.
    pub fn settings(&self) -> AppSettings {
        self.settings
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Открывает транзакцию для изменения данных. ВСЕ записи в БД обязаны идти через неё
    /// (или через `begin_write_on`, когда `Core` ещё не создан).
    pub(crate) async fn begin_write(&self) -> Result<DatabaseTransaction, CoreError> {
        begin_write_on(&self.db).await
    }
}

/// Глобальная блокировка единственного писателя в PostgreSQL: снимается сама при commit/rollback,
/// повторяет поведение SQLite, где писатель всегда один.
const WRITE_LOCK_KEY: i64 = 0x6D77_0001;

/// Единственный способ открыть транзакцию записи.
///
/// В SQLite транзакция начинается в режиме IMMEDIATE: в WAL-режиме с пулом соединений
/// «прочитать, затем записать» в обычной (отложенной) транзакции даёт `SQLITE_BUSY`,
/// который `busy_timeout` не гасит. В PostgreSQL сразу после BEGIN берётся advisory-блокировка
/// `WRITE_LOCK_KEY`, поэтому писатели идут строго по одному, как в SQLite.
pub(crate) async fn begin_write_on(
    db: &DatabaseConnection,
) -> Result<DatabaseTransaction, CoreError> {
    let tx = match db.get_database_backend() {
        DatabaseBackend::Sqlite => {
            db.begin_with_options(TransactionOptions {
                sqlite_transaction_mode: Some(sea_orm::SqliteTransactionMode::Immediate),
                ..Default::default()
            })
            .await?
        }
        DatabaseBackend::Postgres => {
            // READ COMMITTED явно: при REPEATABLE READ снимок берётся самим запросом блокировки
            // ДО её получения, и писатель не увидит коммит предыдущего владельца блокировки.
            let tx = db
                .begin_with_config(Some(IsolationLevel::ReadCommitted), None)
                .await?;
            tx.execute_unprepared(&format!("SELECT pg_advisory_xact_lock({WRITE_LOCK_KEY})"))
                .await?;
            tx
        }
        _ => db.begin().await?,
    };
    Ok(tx)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time::timeout;

    use super::*;

    #[tokio::test]
    async fn second_writer_waits_for_first_to_finish() {
        for (label, db) in mw_db::testing::all_backends().await {
            let first = begin_write_on(&db).await.unwrap();
            let db2 = db.clone();
            let mut second = tokio::spawn(async move {
                let tx = begin_write_on(&db2).await.unwrap();
                tx.commit().await.unwrap();
            });
            assert!(
                timeout(Duration::from_millis(300), &mut second)
                    .await
                    .is_err(),
                "{label}: второй писатель не должен начать, пока открыта первая транзакция"
            );
            first.commit().await.unwrap();
            timeout(Duration::from_secs(10), second)
                .await
                .unwrap_or_else(|_| panic!("{label}: второй писатель завис"))
                .unwrap();
        }
    }

    #[tokio::test]
    async fn postgres_write_transaction_is_read_committed_even_if_server_default_differs() {
        use sea_orm::{ConnectOptions, Database, DbBackend, Statement};
        let Ok(url) = std::env::var("MW_TEST_PG_URL") else {
            return;
        };
        let mut options = ConnectOptions::new(url);
        options.max_connections(1).sqlx_logging(false);
        options.map_sqlx_postgres_opts(|o| {
            o.options([("default_transaction_isolation", "repeatable read")])
        });
        let db = Database::connect(options).await.unwrap();
        let tx = begin_write_on(&db).await.unwrap();
        let row = tx
            .query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                "SHOW transaction_isolation",
            ))
            .await
            .unwrap()
            .unwrap();
        let level: String = row.try_get_by_index(0).unwrap();
        assert_eq!(level, "read committed");
        tx.commit().await.unwrap();
    }
}
