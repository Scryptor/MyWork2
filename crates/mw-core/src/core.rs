use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use chrono::{NaiveDateTime, SubsecRound, TimeZone};
use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction, IsolationLevel,
    TransactionOptions, TransactionTrait,
};

use tokio_util::task::TaskTracker;

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
    /// Корень папки данных: файлы вложений лежат в `<корень>/attachments`.
    pub(crate) data_dir: Option<PathBuf>,
    /// Фоновые задачи, которые нельзя бросать при остановке (перенос файлов черновиков).
    pub(crate) background: TaskTracker,
    /// Порядок обновления кэша настроек (см. `Core::commit_with_settings`).
    pub(crate) settings_gate: Arc<tokio::sync::Mutex<()>>,
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
            data_dir: None,
            background: TaskTracker::new(),
            settings_gate: Arc::new(tokio::sync::Mutex::new(())),
        })
    }

    /// Задаёт папку данных; без неё операции с файлами вложений недоступны.
    pub fn with_attachments_dir(mut self, data_dir: PathBuf) -> Core {
        self.data_dir = Some(data_dir);
        self
    }

    /// Папка данных, если сервер её задал.
    pub fn data_dir_path(&self) -> Option<&std::path::Path> {
        self.data_dir.as_deref()
    }

    /// Название СУБД для людей: «SQLite» или «PostgreSQL».
    pub fn database_label(&self) -> &'static str {
        match self.db.get_database_backend() {
            DatabaseBackend::Postgres => "PostgreSQL",
            _ => "SQLite",
        }
    }

    #[cfg(any(test, feature = "testing"))]
    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    /// Местное время предприятия, до секунд. Единственный источник «сейчас» в приложении.
    pub fn now(&self) -> NaiveDateTime {
        self.local_time(self.clock.now_utc())
    }

    /// Момент UTC по местным часам предприятия, до секунд: так же, как записывает `now()`.
    pub(crate) fn local_time(&self, at: chrono::DateTime<chrono::Utc>) -> NaiveDateTime {
        let tz = settings::parse_timezone(&self.settings().timezone);
        tz.from_utc_datetime(&at.naive_utc())
            .naive_local()
            .trunc_subsecs(0)
    }

    /// Показания часов `Core` в UTC, без перевода в пояс предприятия (интервалы, ограничение попыток).
    pub fn now_utc(&self) -> chrono::DateTime<chrono::Utc> {
        self.clock.now_utc()
    }

    /// Ждёт фоновые задачи (перенос файлов черновиков): вызывается перед закрытием БД.
    pub async fn wait_background(&self) {
        self.background.close();
        self.background.wait().await;
        self.background.reopen();
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
