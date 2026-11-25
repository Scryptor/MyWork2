//! Отчёты по заказам и перемещениям.

pub(crate) mod moves;
pub(crate) mod orders;

use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime};
use mw_domain::validation::FieldError;

use crate::CoreError;

pub use moves::{MoveItem, MoveRoute, MovesFilter, MovesReport};
pub use orders::{
    OrderReport, OrderReportGroup, ReportFilter, ReportGroupBy, ReportPoint, ReportStep,
    ReportTotals,
};

/// Период в одном отчёте не может кончаться раньше начала: `period_reversed` у поля `to`.
pub(crate) fn check_period(
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> Result<(), CoreError> {
    if let (Some(from), Some(to)) = (from, to)
        && from > to
    {
        return Err(CoreError::Validation(vec![FieldError::new(
            "to",
            "period_reversed",
        )]));
    }
    Ok(())
}

/// Границы периода по местному времени: «не раньше `from` 00:00» и «раньше 00:00 следующего за `to`
/// дня». Год 10000 в SQLite записывается пятью цифрами и сравнивается как текст неверно: после
/// 9999-12-31 верхней границы нет.
pub(crate) fn period_bounds(
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> (Option<NaiveDateTime>, Option<NaiveDateTime>) {
    (
        from.map(|d| d.and_time(NaiveTime::MIN)),
        to.and_then(|to| to.succ_opt())
            .filter(|next| next.year() <= 9999)
            .map(|d| d.and_time(NaiveTime::MIN)),
    )
}

/// Название точки в отчёте: архивная — с пометкой.
pub(crate) fn point_title(name: &str, archived: bool) -> String {
    if archived {
        format!("{name} (в архиве)")
    } else {
        name.to_owned()
    }
}
