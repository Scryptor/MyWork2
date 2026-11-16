//! Пересчёт производных колонок по журналу.

use sea_orm::{ConnectionTrait, DbErr, Statement};

/// `orders.status_changed_at` = время последней смены статуса из журнала (в том числе
/// записи импорта `legacy.status`); без записей — пусто. Возвращает число обновлённых строк.
pub async fn status_changed_at(conn: &impl ConnectionTrait) -> Result<u64, DbErr> {
    let sql = "UPDATE orders SET status_changed_at = (SELECT MAX(a.at) FROM audit_log a \
               WHERE a.order_id = orders.id AND a.action IN ('order.status_changed', 'legacy.status'))";
    let res = conn
        .execute_raw(Statement::from_string(conn.get_database_backend(), sql))
        .await?;
    Ok(res.rows_affected())
}
