//! Нумерация: номер заказа = `orders.id` (номер позиции склада — `stock_items.id`), поэтому
//! «следующий номер» — это последовательность таблицы.

use sea_orm::{ConnectionTrait, DatabaseBackend, DbErr, Statement};

/// Таблицы с управляемой нумерацией. Имя таблицы берётся только отсюда.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdTable {
    Orders,
    Clients,
    StockItems,
}

impl IdTable {
    fn table(self) -> &'static str {
        match self {
            Self::Orders => "orders",
            Self::Clients => "clients",
            Self::StockItems => "stock_items",
        }
    }
}

/// Наибольший `id` таблицы; 0 для пустой.
pub async fn max_id<C: ConnectionTrait>(db: &C, table: IdTable) -> Result<i64, DbErr> {
    let sql = format!("SELECT COALESCE(MAX(id), 0) FROM {}", table.table());
    let row = db
        .query_one_raw(Statement::from_string(db.get_database_backend(), sql))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("MAX(id)".to_owned()))?;
    row.try_get_by_index::<i64>(0)
}

/// Следующий автоматический `id` таблицы будет `next` (не меньше 1).
///
/// Если в таблице уже есть строки с `id >= next`, SQLite всё равно выдаст `max(id) + 1`,
/// PostgreSQL же упрётся в дубликат: вызывающий обязан выбирать `next` выше `max_id`.
/// Принимает и соединение, и транзакцию: внутри открытой транзакции не открывает новых соединений.
pub async fn set_next_id<C: ConnectionTrait>(
    db: &C,
    table: IdTable,
    next: i64,
) -> Result<(), DbErr> {
    if next < 1 {
        return Err(DbErr::Custom(
            "следующий номер должен быть не меньше 1".to_owned(),
        ));
    }
    let name = table.table();
    match db.get_database_backend() {
        DatabaseBackend::Sqlite => {
            let backend = DatabaseBackend::Sqlite;
            let updated = db
                .execute_raw(Statement::from_sql_and_values(
                    backend,
                    "UPDATE sqlite_sequence SET seq = ? WHERE name = ?",
                    [(next - 1).into(), name.into()],
                ))
                .await?;
            if updated.rows_affected() == 0 {
                db.execute_raw(Statement::from_sql_and_values(
                    backend,
                    "INSERT INTO sqlite_sequence (name, seq) VALUES (?, ?)",
                    [name.into(), (next - 1).into()],
                ))
                .await?;
            }
            Ok(())
        }
        DatabaseBackend::Postgres => {
            db.execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT setval(pg_get_serial_sequence($1, 'id'), $2, false)",
                [name.into(), next.into()],
            ))
            .await?;
            Ok(())
        }
        other => Err(DbErr::Custom(format!("неподдерживаемая СУБД: {other:?}"))),
    }
}

/// Какой `id` получит следующая автоматически созданная строка таблицы.
pub async fn next_id<C: ConnectionTrait>(db: &C, table: IdTable) -> Result<i64, DbErr> {
    let name = table.table();
    match db.get_database_backend() {
        DatabaseBackend::Sqlite => {
            let seq = db
                .query_one_raw(Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    "SELECT seq FROM sqlite_sequence WHERE name = ?",
                    [name.into()],
                ))
                .await?
                .map(|row| row.try_get_by_index::<i64>(0))
                .transpose()?
                .unwrap_or(0);
            Ok(seq.max(max_id(db, table).await?) + 1)
        }
        DatabaseBackend::Postgres => {
            let sequence = db
                .query_one_raw(Statement::from_sql_and_values(
                    DatabaseBackend::Postgres,
                    "SELECT pg_get_serial_sequence($1, 'id')",
                    [name.into()],
                ))
                .await?
                .ok_or_else(|| DbErr::RecordNotFound(format!("последовательность {name}")))?
                .try_get_by_index::<String>(0)?;
            // Имя вернула сама СУБД, уже с кавычками, где они нужны.
            let row = db
                .query_one_raw(Statement::from_string(
                    DatabaseBackend::Postgres,
                    format!(
                        "SELECT CASE WHEN is_called THEN last_value + 1 ELSE last_value END FROM {sequence}"
                    ),
                ))
                .await?
                .ok_or_else(|| DbErr::RecordNotFound(format!("последовательность {name}")))?;
            row.try_get_by_index::<i64>(0)
        }
        other => Err(DbErr::Custom(format!("неподдерживаемая СУБД: {other:?}"))),
    }
}
