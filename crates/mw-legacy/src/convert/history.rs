//! Журнал: `StatesMap` и `HistoryBD` → записи журнала со своим временем.

use std::collections::{BTreeMap, HashMap};

use chrono::NaiveDateTime;
use mw_core::{ImportedHistory, ImportedOrder};
use mw_domain::search::normalize_search;
use serde_json::{Value, json};

use crate::parse::{DateValue, StateEvent, parse_date, read_states_row};
use crate::report::{WarningCode, Warnings};
use crate::source::{LegacyDb, LegacyTable, LegacyValue};

/// Подсказки для заказов, извлечённые из истории.
#[derive(Debug, Default)]
pub(crate) struct HistoryHints {
    /// Самая ранняя разобранная дата записи истории заказа.
    pub earliest: BTreeMap<i64, NaiveDateTime>,
    /// Последняя запись «УДАЛЕНИЕ» заказа.
    pub deleted: BTreeMap<i64, NaiveDateTime>,
}

fn order_ref(text: &str) -> Option<i64> {
    text.trim().parse().ok()
}

fn known(value: DateValue) -> Option<NaiveDateTime> {
    match value {
        DateValue::Parsed(d) | DateValue::MonthFirst(d) => Some(d),
        DateValue::Empty | DateValue::Invalid => None,
    }
}

fn states_at(table: &LegacyTable, row: &[LegacyValue]) -> DateValue {
    read_states_row(&table.text(row, "State"), &table.text(row, "date")).at
}

pub(crate) fn history_hints(db: &LegacyDb) -> HistoryHints {
    let mut hints = HistoryHints::default();
    let note = |hints: &mut HistoryHints, order: Option<i64>, at: Option<NaiveDateTime>| {
        if let (Some(order), Some(at)) = (order, at) {
            hints
                .earliest
                .entry(order)
                .and_modify(|e| *e = (*e).min(at))
                .or_insert(at);
        }
    };
    if let Some(table) = db.table("StatesMap") {
        for row in &table.rows {
            let order = order_ref(&table.text(row, "clientId"));
            note(&mut hints, order, known(states_at(table, row)));
        }
    }
    if let Some(table) = db.table("HistoryBD") {
        for row in &table.rows {
            let order = order_ref(&table.text(row, "IDINCATALOG"));
            let at = known(parse_date(&table.text(row, "DATA")));
            note(&mut hints, order, at);
            if let (Some(order), Some(at)) = (order, at)
                && normalize_search(&table.text(row, "WHAT")) == "удаление"
            {
                hints
                    .deleted
                    .entry(order)
                    .and_modify(|e| *e = (*e).max(at))
                    .or_insert(at);
            }
        }
    }
    hints
}

struct Known {
    received_at: NaiveDateTime,
    client_id: i64,
}

struct Entry {
    at: NaiveDateTime,
    table: &'static str,
    id: i64,
    history: ImportedHistory,
}

fn details(table: &str, id_text: &str, row: Value) -> Value {
    let id = id_text
        .trim()
        .parse::<i64>()
        .map_or_else(|_| Value::from(id_text), Value::from);
    json!({ "legacy": { "table": table, "id": id, "row": row } })
}

/// Время записи: разобранное или дата приёма заказа, иначе время импорта.
fn resolve_at(
    value: DateValue,
    table: &'static str,
    id: &str,
    shown: &str,
    order: Option<&Known>,
    now: NaiveDateTime,
    w: &mut Warnings,
) -> NaiveDateTime {
    let field = if table == "StatesMap" { "date" } else { "DATA" };
    match value {
        DateValue::Parsed(d) => d,
        DateValue::MonthFirst(d) => {
            w.push(
                WarningCode::DateMonthFirst,
                table,
                id,
                field,
                shown,
                "Дата с месяцем больше 12 прочитана как «месяц/день».",
            );
            d
        }
        DateValue::Empty | DateValue::Invalid => {
            w.push(
                WarningCode::DateUnparsed,
                table,
                id,
                field,
                shown,
                "Дата записи не разобрана: взята дата приёма заказа или время импорта.",
            );
            order.map_or(now, |o| o.received_at)
        }
    }
}

fn states_entries(
    db: &LegacyDb,
    orders: &HashMap<i64, Known>,
    now: NaiveDateTime,
    w: &mut Warnings,
    out: &mut Vec<Entry>,
) {
    let Some(table) = db.table("StatesMap") else {
        return;
    };
    for row in &table.rows {
        let id = table.text(row, "id");
        let (state, date) = (table.text(row, "State"), table.text(row, "date"));
        let parsed = read_states_row(&state, &date);
        let order_id = order_ref(&table.text(row, "clientId"));
        let order = order_id.and_then(|o| orders.get(&o));
        if order.is_none() {
            w.push(
                WarningCode::HistoryOrphan,
                "StatesMap",
                &id,
                "clientId",
                &table.text(row, "clientId"),
                "Записи истории нет заказа с таким номером; она перенесена без привязки к заказу.",
            );
        }
        let shown = format!("{} | {}", state.trim(), date.trim());
        let at = resolve_at(parsed.at, "StatesMap", &id, &shown, order, now, w);
        let (action, summary) = match &parsed.event {
            StateEvent::Status(v) => ("legacy.status", format!("Установлен статус «{v}»")),
            StateEvent::Master(v) => ("legacy.master", format!("Назначен мастер «{v}»")),
            StateEvent::FlagSet(v) => ("legacy.flag", format!("Установлена галочка «{v}»")),
            StateEvent::FlagCleared(v) => ("legacy.flag", format!("Снята галочка «{v}»")),
            StateEvent::Message(_) => ("legacy.message", "Отправлено сообщение".to_owned()),
            StateEvent::Other(text) => ("legacy.event", text.clone()),
        };
        out.push(Entry {
            at,
            table: "StatesMap",
            id: order_ref(&id).unwrap_or(i64::MAX),
            history: ImportedHistory {
                at,
                user_name: String::new(),
                action: action.to_owned(),
                order_id: order_id.filter(|_| order.is_some()),
                client_id: order.map(|o| o.client_id),
                summary,
                details: Some(details("StatesMap", &id, table.row_json(row))),
            },
        });
    }
}

fn history_entries(
    db: &LegacyDb,
    orders: &HashMap<i64, Known>,
    now: NaiveDateTime,
    w: &mut Warnings,
    out: &mut Vec<Entry>,
) {
    let Some(table) = db.table("HistoryBD") else {
        return;
    };
    for row in &table.rows {
        let id = table.text(row, "id");
        let reference = table.text(row, "IDINCATALOG");
        let order_id = order_ref(&reference);
        let order = order_id.and_then(|o| orders.get(&o));
        if order.is_none() && !reference.trim().is_empty() {
            w.push(
                WarningCode::HistoryOrphan,
                "HistoryBD",
                &id,
                "IDINCATALOG",
                &reference,
                "В истории указан несуществующий заказ; запись перенесена без привязки к заказу.",
            );
        }
        let date = table.text(row, "DATA");
        let at = resolve_at(parse_date(&date), "HistoryBD", &id, &date, order, now, w);
        out.push(Entry {
            at,
            table: "HistoryBD",
            id: order_ref(&id).unwrap_or(i64::MAX),
            history: ImportedHistory {
                at,
                user_name: table.text(row, "WHO"),
                action: "legacy.history".to_owned(),
                order_id: order_id.filter(|_| order.is_some()),
                client_id: order.map(|o| o.client_id),
                summary: table.text(row, "WHAT"),
                details: Some(details("HistoryBD", &id, table.row_json(row))),
            },
        });
    }
}

/// Все записи журнала по возрастанию (время, таблица, номер).
pub(crate) fn convert_history(
    db: &LegacyDb,
    orders: &[ImportedOrder],
    now: NaiveDateTime,
    w: &mut Warnings,
) -> Vec<ImportedHistory> {
    let known: HashMap<i64, Known> = orders
        .iter()
        .map(|o| {
            (
                o.id,
                Known {
                    received_at: o.received_at,
                    client_id: o.client_id,
                },
            )
        })
        .collect();
    let mut entries = Vec::new();
    states_entries(db, &known, now, w, &mut entries);
    history_entries(db, &known, now, w, &mut entries);
    entries.sort_by_key(|e| (e.at, e.table, e.id));
    entries.into_iter().map(|e| e.history).collect()
}
