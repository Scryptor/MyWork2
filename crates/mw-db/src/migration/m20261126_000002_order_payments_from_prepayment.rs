//! Перенос предоплаты заказов в платежи: каждому заказу с `prepayment > 0` и без платежей — один
//! платёж этапа «до выдачи». Поле `orders.prepayment` не меняется.

use chrono::NaiveDateTime;
use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261126_000002_order_payments_from_prepayment"
    }
}

fn a(name: &str) -> Alias {
    Alias::new(name)
}

/// 10 колонок × 500 строк — меньше пределов параметров SQLite и PostgreSQL.
const CHUNK: usize = 500;

// Литерал этапа ЗАМОРОЖЕН: правка `mw_domain::payment` выпущенную миграцию не меняет.
const STAGE: &str = "before_issue";

struct Row {
    order_id: i64,
    amount: i64,
    stamp: NaiveDateTime,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        let mut select = Query::select();
        select
            .columns([a("id"), a("prepayment"), a("prepaid_at"), a("received_at")])
            .from(a("orders"))
            .and_where(Expr::col(a("prepayment")).gt(0))
            .and_where(Expr::cust(
                "NOT EXISTS (SELECT 1 FROM order_payments WHERE order_payments.order_id = orders.id)",
            ))
            .order_by(a("id"), Order::Asc);
        let mut rows = Vec::new();
        for row in db.query_all(&select).await? {
            let prepaid_at: Option<NaiveDateTime> = row.try_get_by_index(2)?;
            let received_at: NaiveDateTime = row.try_get_by_index(3)?;
            rows.push(Row {
                order_id: row.try_get_by_index(0)?,
                amount: row.try_get_by_index(1)?,
                stamp: prepaid_at.unwrap_or(received_at),
            });
        }
        for chunk in rows.chunks(CHUNK) {
            let mut insert = Query::insert();
            insert.into_table(a("order_payments")).columns([
                a("order_id"),
                a("day"),
                a("amount"),
                a("received_by"),
                a("stage"),
                a("added_by_id"),
                a("added_by_name"),
                a("added_at"),
                a("legacy"),
                a("request_key"),
            ]);
            for r in chunk {
                insert
                    .values([
                        r.order_id.into(),
                        r.stamp.date().into(),
                        r.amount.into(),
                        "".into(),
                        STAGE.into(),
                        Option::<i64>::None.into(),
                        "".into(),
                        r.stamp.into(),
                        true.into(),
                        Option::<String>::None.into(),
                    ])
                    .map_err(|e| DbErr::Migration(format!("перенос предоплаты: {e}")))?;
            }
            m.exec_stmt(insert).await?;
        }
        Ok(())
    }
}
