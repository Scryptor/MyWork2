//! Строки журнала статусов (`StatesMap`): колонки в большинстве строк поменяны местами.

use super::dates::{DateValue, parse_date};

/// Что произошло.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StateEvent {
    Status(String),
    Master(String),
    FlagSet(String),
    FlagCleared(String),
    Message(String),
    Other(String),
}

/// Строка журнала: когда и что.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StatesRow {
    pub at: DateValue,
    pub event: StateEvent,
}

fn is_date(v: DateValue) -> bool {
    matches!(v, DateValue::Parsed(_) | DateValue::MonthFirst(_))
}

/// Определяет ориентацию колонок и классифицирует текст.
pub(crate) fn read_states_row(state: &str, date: &str) -> StatesRow {
    let from_state = parse_date(state);
    if is_date(from_state) {
        return StatesRow {
            at: from_state,
            event: classify(date),
        };
    }
    let from_date = parse_date(date);
    if is_date(from_date) {
        return StatesRow {
            at: from_date,
            event: StateEvent::Status(state.trim().to_owned()),
        };
    }
    let text = if date.trim().is_empty() { state } else { date };
    StatesRow {
        at: DateValue::Invalid,
        event: classify(text),
    }
}

/// Первая непустая строка — вид события, всё остальное (с краёв без пробелов) — значение.
fn classify(text: &str) -> StateEvent {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let trimmed = normalized.trim();
    let (head, value) = trimmed.split_once('\n').unwrap_or((trimmed, ""));
    let (head, value) = (head.trim(), value.trim());
    if value.is_empty() {
        return StateEvent::Other(text.to_owned());
    }
    let value = value.to_owned();
    match head {
        "Установлен статус" => StateEvent::Status(value),
        "Назначен мастер" => StateEvent::Master(value),
        "Установлена галочка" => StateEvent::FlagSet(value),
        "Снята галочка" => StateEvent::FlagCleared(value),
        "Сообщение отправлено" => StateEvent::Message(value),
        _ => StateEvent::Other(text.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDateTime;

    fn dt(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").unwrap()
    }

    #[test]
    fn reads_both_orientations() {
        let r = read_states_row("05-03-2015 14-20", "Установлен статус\r\nДиагностика");
        assert_eq!(
            (r.at, r.event),
            (
                DateValue::Parsed(dt("2015-03-05 14:20:00")),
                StateEvent::Status("Диагностика".into())
            )
        );
        assert_eq!(
            read_states_row("Ждет ЗИП", "06-03-2015 10:00").event,
            StateEvent::Status("Ждет ЗИП".into())
        );
    }

    #[test]
    fn classifies_text() {
        assert_eq!(
            read_states_row("06-03-2015 09:10", "Назначен мастер \r\nПЕТРОВ").event,
            StateEvent::Master("ПЕТРОВ".into())
        );
        assert_eq!(
            read_states_row("x", "Установлена галочка\r\nТребует заказа").event,
            StateEvent::FlagSet("Требует заказа".into())
        );
        assert_eq!(
            read_states_row("x", "Снята галочка\nНужно позвонить").event,
            StateEvent::FlagCleared("Нужно позвонить".into())
        );
        assert_eq!(
            read_states_row("x", "Сообщение отправлено\r\nВаш заказ готов").event,
            StateEvent::Message("Ваш заказ готов".into())
        );
        assert_eq!(
            read_states_row("мусор", "Установлен статус\r\nГотов").at,
            DateValue::Invalid
        );
        assert_eq!(
            read_states_row("07-03-2015 12-00", "").event,
            StateEvent::Other(String::new())
        );
    }

    #[test]
    fn keeps_all_lines_of_the_value() {
        assert_eq!(
            read_states_row("x", "Сообщение отправлено\r\nСтрока 1\r\nСтрока 2").event,
            StateEvent::Message("Строка 1\nСтрока 2".into())
        );
    }

    #[test]
    fn tolerates_blank_lines_and_missing_value() {
        assert_eq!(
            read_states_row("x", "Установлен статус\r\n\r\n Готов ").event,
            StateEvent::Status("Готов".into())
        );
        assert_eq!(
            read_states_row("07-03-2015 12-00", "Установлен статус").event,
            StateEvent::Other("Установлен статус".into())
        );
        assert_eq!(read_states_row("", "").at, DateValue::Invalid);
    }
}
