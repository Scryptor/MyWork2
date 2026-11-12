//! Даты в форматах, которые писала MyWork 2, и общий разбор «день первым».

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

/// Результат разбора даты.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DateValue {
    /// Пустая строка.
    Empty,
    /// Дата разобрана.
    Parsed(NaiveDateTime),
    /// Дата через «/» с месяцем больше 12 прочитана как «месяц/день».
    MonthFirst(NaiveDateTime),
    /// Не дата.
    Invalid,
}

/// Разбирает дату. Год 1990–2100, дата без времени — 00:00.
pub(crate) fn parse_date(raw: &str) -> DateValue {
    let text = raw.trim();
    if text.is_empty() {
        return DateValue::Empty;
    }
    parse_non_empty(text).unwrap_or(DateValue::Invalid)
}

fn parse_non_empty(text: &str) -> Option<DateValue> {
    let (date_text, time_text) = split_date_time(text);
    let (date, month_first) = parse_date_part(date_text)?;
    let time = match time_text {
        None => NaiveTime::MIN,
        Some(t) => parse_time_part(t)?,
    };
    let value = date.and_time(time);
    Some(if month_first {
        DateValue::MonthFirst(value)
    } else {
        DateValue::Parsed(value)
    })
}

/// Делит на дату и время по `T` или пробелу; AM/PM остаётся в части времени.
fn split_date_time(text: &str) -> (&str, Option<&str>) {
    match text.find(|c: char| c == 'T' || c.is_whitespace()) {
        Some(i) => {
            let rest = text[i..].trim_start_matches(|c: char| c == 'T' || c.is_whitespace());
            (&text[..i], Some(rest))
        }
        None => (text, None),
    }
}

fn parse_date_part(text: &str) -> Option<(NaiveDate, bool)> {
    let sep = text.chars().find(|c| matches!(c, '-' | '.' | '/'))?;
    let parts: Vec<&str> = text.split(sep).collect();
    let [a, b, c] = parts[..] else { return None };
    let (first, second, third) = (number(a)?, number(b)?, number(c)?);
    let (year, month, day, month_first) = if a.len() == 4 {
        (first, second, third, false)
    } else if sep == '/' && second > 12 && first <= 12 {
        (expand_year(c, third)?, first, second, true)
    } else {
        (expand_year(c, third)?, second, first, false)
    };
    if !(1990..=2100).contains(&year) {
        return None;
    }
    let date = NaiveDate::from_ymd_opt(i32::try_from(year).ok()?, month, day)?;
    Some((date, month_first))
}

fn number(s: &str) -> Option<u32> {
    if s.is_empty() || s.len() > 4 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// Двузначный год — 20xx.
fn expand_year(text: &str, value: u32) -> Option<u32> {
    match text.len() {
        2 => Some(2000 + value),
        4 => Some(value),
        _ => None,
    }
}

fn parse_time_part(text: &str) -> Option<NaiveTime> {
    let upper = text.trim().to_uppercase();
    let (body, meridiem) = if let Some(b) = upper.strip_suffix("AM") {
        (b, Some(false))
    } else if let Some(b) = upper.strip_suffix("PM") {
        (b, Some(true))
    } else {
        (upper.as_str(), None)
    };
    let body = body.trim();
    let sep = if body.contains(':') { ':' } else { '-' };
    let parts: Vec<&str> = body.split(sep).collect();
    let (h, m, s) = match parts[..] {
        [h, m] => (number(h)?, number(m)?, 0),
        [h, m, s] => (number(h)?, number(m)?, number(s)?),
        _ => return None,
    };
    let hour = match meridiem {
        None => h,
        Some(pm) => {
            if !(1..=12).contains(&h) {
                return None;
            }
            (h % 12) + if pm { 12 } else { 0 }
        }
    };
    NaiveTime::from_hms_opt(hour, m, s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").unwrap()
    }

    #[test]
    fn parses_every_format_of_the_old_program() {
        assert_eq!(
            parse_date("05-03-2015 14:20"),
            DateValue::Parsed(dt("2015-03-05 14:20:00"))
        );
        assert_eq!(
            parse_date("05-03-2015 14-20"),
            DateValue::Parsed(dt("2015-03-05 14:20:00"))
        );
        assert_eq!(
            parse_date("2014-11-02"),
            DateValue::Parsed(dt("2014-11-02 00:00:00"))
        );
        assert_eq!(
            parse_date("05.03.2015"),
            DateValue::Parsed(dt("2015-03-05 00:00:00"))
        );
        assert_eq!(
            parse_date("12.03.2015 15:04:05"),
            DateValue::Parsed(dt("2015-03-12 15:04:05"))
        );
        assert_eq!(
            parse_date("5.3.15 9:05"),
            DateValue::Parsed(dt("2015-03-05 09:05:00"))
        );
        assert_eq!(
            parse_date("2015-03-05T14:20:00"),
            DateValue::Parsed(dt("2015-03-05 14:20:00"))
        );
        assert_eq!(
            parse_date("03/04/2015"),
            DateValue::Parsed(dt("2015-04-03 00:00:00"))
        );
        assert_eq!(
            parse_date("13/03/2015"),
            DateValue::Parsed(dt("2015-03-13 00:00:00"))
        );
        assert_eq!(
            parse_date("03/20/2015 9:15:00 PM"),
            DateValue::MonthFirst(dt("2015-03-20 21:15:00"))
        );
    }

    #[test]
    fn empty_and_invalid() {
        assert_eq!(parse_date("  "), DateValue::Empty);
        assert_eq!(parse_date(""), DateValue::Empty);
        for bad in [
            "вчера",
            "31-02-2015 10:00",
            "31-13-2015 10:00",
            "01-01-0001 00:00",
            "05-03-2015 25:00",
            "2015",
            "05-03-2015 14:20:00:00",
            "05-03-2015 13:00 PM",
            "05-03-15x",
            "-",
        ] {
            assert_eq!(parse_date(bad), DateValue::Invalid, "{bad}");
        }
    }

    #[test]
    fn tolerates_spaces_and_nbsp() {
        assert_eq!(
            parse_date("\u{a0}05-03-2015\u{a0}14:20 "),
            DateValue::Parsed(dt("2015-03-05 14:20:00"))
        );
        assert_eq!(parse_date("12 AM"), DateValue::Invalid);
        assert_eq!(
            parse_date("01/02/2015 12:00 AM"),
            DateValue::Parsed(dt("2015-02-01 00:00:00"))
        );
    }
}
