use std::sync::{Arc, RwLock};

use chrono::{NaiveDateTime, SubsecRound, TimeZone};
use sea_orm::{
    DatabaseBackend, DatabaseConnection, DatabaseTransaction, TransactionOptions, TransactionTrait,
};

use crate::settings::{self, AppSettings};
use crate::{Clock, CoreError};

/// Приложение: БД, часы и кэш настроек. Дёшево клонируется.
#[derive(Clone)]
pub struct Core {
    pub(crate) db: DatabaseConnection,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) settings: Arc<RwLock<AppSettings>>,
}

impl Core {
    /// Если настройки `timezone` ещё нет, сохраняет `system_timezone` (неверное имя → `"UTC"`).
    pub async fn open(
        db: DatabaseConnection,
        clock: Arc<dyn Clock>,
        system_timezone: &str,
    ) -> Result<Core, CoreError> {
        let loaded = settings::ensure_timezone(&db, system_timezone).await?;
        Ok(Core {
            db,
            clock,
            settings: Arc::new(RwLock::new(loaded)),
        })
    }

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

/// Единственный способ открыть транзакцию записи.
///
/// В SQLite транзакция начинается в режиме IMMEDIATE: в WAL-режиме с пулом соединений
/// «прочитать, затем записать» в обычной (отложенной) транзакции даёт `SQLITE_BUSY`,
/// который `busy_timeout` не гасит. В PostgreSQL — обычная транзакция.
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
        _ => db.begin().await?,
    };
    Ok(tx)
}
