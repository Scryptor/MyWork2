//! Мелкие помощники запросов, одинаково работающие на SQLite и PostgreSQL.

use sea_orm::DatabaseBackend;
use sea_orm::sea_query::{Expr, SimpleExpr};

/// Выражение для `ORDER BY` по тексту в двоичном порядке кодовых точек на обеих СУБД.
///
/// SQLite сравнивает текст двоично (`BINARY`) по умолчанию; в PostgreSQL порядок зависит от
/// локали базы, поэтому там добавляется `COLLATE "C"`.
pub(crate) fn text_sort(backend: DatabaseBackend, expr: impl Into<SimpleExpr>) -> SimpleExpr {
    match backend {
        DatabaseBackend::Postgres => Expr::cust_with_expr("$1 COLLATE \"C\"", expr.into()),
        _ => expr.into(),
    }
}
/// Сколько номеров кладётся в один `IN (…)`: SQLite принимает в запросе не больше 32 766
/// переменных, а выгрузки берут до 50 000 строк.
pub(crate) const ID_CHUNK: usize = 500;

#[cfg(test)]
mod tests {
    use sea_orm::sea_query::{Order, PostgresQueryBuilder, Query};

    use super::*;

    #[test]
    fn postgres_sort_gets_binary_collation_and_sqlite_does_not() {
        let sql = |backend| {
            let mut q = Query::select();
            q.column("a")
                .from("t")
                .order_by_expr(text_sort(backend, Expr::col("x")), Order::Asc);
            q.to_string(PostgresQueryBuilder)
        };
        assert_eq!(
            sql(DatabaseBackend::Postgres),
            r#"SELECT "a" FROM "t" ORDER BY "x" COLLATE "C" ASC"#
        );
        assert_eq!(
            sql(DatabaseBackend::Sqlite),
            r#"SELECT "a" FROM "t" ORDER BY "x" ASC"#
        );
    }
}
