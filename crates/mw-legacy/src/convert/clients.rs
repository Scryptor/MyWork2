//! `ClientsMap` и клиенты, восстановленные из заказов.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::NaiveDateTime;
use mw_core::{
    ClientPhone, ImportedClient, ImportedLegacyRow, ImportedOrder, LegacyTarget, MAX_CLIENT_PHONES,
};
use mw_domain::phone::{normalize_phone, phone_key};
use mw_domain::search::normalize_search;

use crate::convert::orders::catalog_rows;
use crate::parse::{DateValue, parse_date, parse_flag, split_phones};
use crate::report::{WarningCode, Warnings};
use crate::source::LegacyDb;

const NO_NAME: &str = "Клиент без имени";
const NOT_GIVEN: &str = "Клиент не указан";

/// Клиенты и связь заказов с ними.
pub(crate) struct ClientStage {
    pub clients: Vec<ImportedClient>,
    /// Номер заказа → номер клиента.
    pub order_client: BTreeMap<i64, i64>,
    pub rows: Vec<ImportedLegacyRow>,
    pub next_client_id: i64,
    /// Клиенты, у которых дата создания взята из `ClientsMap`.
    dated: BTreeSet<i64>,
}

/// Сырые поля клиента из любого источника.
struct Raw<'a> {
    source: &'a str,
    key: &'a str,
    fio: &'a str,
    phone_field: &'a str,
    address: &'a str,
    note: &'a str,
    about: &'a str,
    blacklisted: bool,
}

fn build_client(w: &mut Warnings, id: i64, raw: &Raw, created_at: NaiveDateTime) -> ImportedClient {
    let split = split_phones(raw.phone_field);
    let push = |w: &mut Warnings, code, value: &str, message: &str| {
        w.push(code, raw.source, raw.key, "Phone", value, message);
    };
    let mut unusual = split.unusual.clone();
    let mut phones = split.phones;
    let mut notes: Vec<String> = Vec::new();
    if !raw.note.trim().is_empty() {
        notes.push(raw.note.trim().to_owned());
    }
    if phones.is_empty() && !split.text_without_number.is_empty() {
        push(
            w,
            WarningCode::PhoneNoDigits,
            raw.phone_field,
            "В поле телефона нет цифр, текст перенесён в заметку клиента.",
        );
        notes.push(format!("Телефон: {}", split.text_without_number));
    }
    if phones.len() > MAX_CLIENT_PHONES {
        let extra: Vec<String> = phones
            .drain(MAX_CLIENT_PHONES..)
            .map(|p| normalize_phone(&p.phone))
            .collect();
        notes.push(format!("Ещё телефоны: {}", extra.join(", ")));
        unusual.extend(extra);
    }
    if !unusual.is_empty() {
        push(
            w,
            WarningCode::PhoneUnusual,
            &unusual.join(", "),
            "Необычная длина номера или слишком много номеров.",
        );
    }
    if split.reshaped {
        push(
            w,
            WarningCode::PhoneSplit,
            raw.phone_field,
            "Поле телефона разложено на отдельные номера и пометки.",
        );
    }
    let mut name = raw.fio.trim().to_owned();
    if name.is_empty() && phones.is_empty() {
        w.push(
            WarningCode::ClientEmpty,
            raw.source,
            raw.key,
            "FIO",
            "",
            "У клиента нет ни имени, ни телефона, он назван «Клиент без имени».",
        );
        name = NO_NAME.to_owned();
    }
    ImportedClient {
        id,
        full_name: name,
        phones,
        address: raw.address.trim().to_owned(),
        note: notes.join("\n"),
        source: raw.about.trim().to_owned(),
        blacklisted: raw.blacklisted,
        created_at,
    }
}

fn sequence(db: &LegacyDb, name: &str) -> i64 {
    db.sequences
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map_or(0, |(_, v)| *v)
}

fn phone_keys(phones: &[ClientPhone]) -> String {
    let keys: Vec<String> = phones.iter().map(|p| phone_key(&p.phone)).collect();
    keys.join(",")
}

/// Ключ объединения: ФИО и телефон после нормализации.
fn merge_key(fio: &str, phone_field: &str) -> String {
    let split = split_phones(phone_field);
    format!(
        "{}|{}|{}",
        normalize_search(fio.trim()),
        phone_keys(&split.phones),
        normalize_search(&split.text_without_number)
    )
}

fn mapped_clients(
    db: &LegacyDb,
    now: NaiveDateTime,
    w: &mut Warnings,
    stage: &mut ClientStage,
) -> Vec<(i64, String)> {
    let Some(table) = db.table("ClientsMap") else {
        return Vec::new();
    };
    let mut keys = Vec::new();
    let mut rows: Vec<_> = table.rows.iter().collect();
    rows.sort_by_key(|r| {
        table
            .text(r, "id")
            .trim()
            .parse::<i64>()
            .unwrap_or(i64::MAX)
    });
    for row in rows {
        let id_text = table.text(row, "id");
        let Ok(id) = id_text.trim().parse::<i64>() else {
            stage.rows.push(ImportedLegacyRow {
                source: "ClientsMap".to_owned(),
                source_key: id_text,
                target: None,
                raw: table.row_json(row),
            });
            continue;
        };
        let key = id.to_string();
        let blist = table.text(row, "Blist");
        let blacklisted = parse_flag(&blist).unwrap_or_else(|| {
            w.push(
                WarningCode::FlagUnrecognized,
                "ClientsMap",
                &key,
                "Blist",
                &blist,
                "Значение флага «чёрный список» не распознано, принято как «нет».",
            );
            false
        });
        let date_raw = table.text(row, "date");
        let created = match parse_date(&date_raw) {
            DateValue::Parsed(d) => Some(d),
            DateValue::MonthFirst(d) => {
                w.push(
                    WarningCode::DateMonthFirst,
                    "ClientsMap",
                    &key,
                    "date",
                    &date_raw,
                    "Дата с месяцем больше 12 прочитана как «месяц/день».",
                );
                Some(d)
            }
            DateValue::Empty => None,
            DateValue::Invalid => {
                w.push(
                    WarningCode::DateUnparsed,
                    "ClientsMap",
                    &key,
                    "date",
                    &date_raw,
                    "Дата создания клиента не разобрана.",
                );
                None
            }
        };
        let raw = Raw {
            source: "ClientsMap",
            key: &key,
            fio: &table.text(row, "FIO"),
            phone_field: &table.text(row, "Phone"),
            address: &table.text(row, "Adress"),
            note: &table.text(row, "Primechanie"),
            about: &table.text(row, "aboutUs"),
            blacklisted,
        };
        let client = build_client(w, id, &raw, created.unwrap_or(now));
        if created.is_some() {
            stage.dated.insert(id);
        }
        if client.full_name != NO_NAME {
            keys.push((id, merge_key(raw.fio, raw.phone_field)));
        }
        stage.rows.push(ImportedLegacyRow {
            source: "ClientsMap".to_owned(),
            source_key: key,
            target: Some(LegacyTarget::Client(id)),
            raw: table.row_json(row),
        });
        stage.clients.push(client);
    }
    keys
}

/// Клиенты из `ClientsMap`, затем восстановленные из заказов (по возрастанию номера заказа).
pub(crate) fn convert_clients(db: &LegacyDb, now: NaiveDateTime, w: &mut Warnings) -> ClientStage {
    let mut stage = ClientStage {
        clients: Vec::new(),
        order_client: BTreeMap::new(),
        rows: Vec::new(),
        next_client_id: 1,
        dated: BTreeSet::new(),
    };
    let mut keys = mapped_clients(db, now, w, &mut stage);
    let mapped: BTreeSet<i64> = stage.clients.iter().map(|c| c.id).collect();
    let mut next = sequence(db, "ClientsMap").max(mapped.last().copied().unwrap_or(0)) + 1;
    if let Some(table) = db.table("Catalog") {
        let has_client_column = table.column("ClientId").is_some();
        let mut restored: HashMap<String, i64> = HashMap::new();
        for (order, row) in catalog_rows(db) {
            let cid_raw = table.text(row, "ClientId");
            if let Ok(cid) = cid_raw.trim().parse::<i64>()
                && mapped.contains(&cid)
            {
                stage.order_client.insert(order, cid);
                continue;
            }
            let (fio, phone) = (table.text(row, "surname"), table.text(row, "phone"));
            let key = order.to_string();
            let has_data = !fio.trim().is_empty() || !phone.trim().is_empty();
            let id = if has_data {
                if has_client_column {
                    w.push(
                        WarningCode::ClientMissing,
                        "Catalog",
                        &key,
                        "ClientId",
                        &cid_raw,
                        "Клиента с таким номером нет в ClientsMap, он восстановлен из полей заказа.",
                    );
                }
                let merge = merge_key(&fio, &phone);
                if let Some(id) = restored.get(&merge) {
                    *id
                } else {
                    let raw = Raw {
                        source: "Catalog",
                        key: &key,
                        fio: &fio,
                        phone_field: &phone,
                        address: &table.text(row, "Adress"),
                        note: "",
                        about: &table.text(row, "AboutUs"),
                        blacklisted: false,
                    };
                    let client = build_client(w, next, &raw, now);
                    keys.push((next, merge.clone()));
                    restored.insert(merge, next);
                    stage.clients.push(client);
                    next += 1;
                    next - 1
                }
            } else {
                w.push(
                    WarningCode::OrderWithoutClient,
                    "Catalog",
                    &key,
                    "ClientId",
                    &cid_raw,
                    "У заказа нет клиента, ни имени, ни телефона: создан «Клиент не указан».",
                );
                stage.clients.push(ImportedClient {
                    id: next,
                    full_name: NOT_GIVEN.to_owned(),
                    phones: Vec::new(),
                    address: String::new(),
                    note: String::new(),
                    source: String::new(),
                    blacklisted: false,
                    created_at: now,
                });
                next += 1;
                next - 1
            };
            stage.order_client.insert(order, id);
        }
    }
    stage.next_client_id = next;
    warn_duplicates(&keys, w);
    stage
}

fn warn_duplicates(keys: &[(i64, String)], w: &mut Warnings) {
    let mut groups: HashMap<&str, usize> = HashMap::new();
    for (_, key) in keys {
        *groups.entry(key).or_default() += 1;
    }
    let count = groups.values().filter(|n| **n > 1).count();
    if count > 0 {
        w.push(
            WarningCode::ClientDuplicates,
            "ClientsMap",
            "",
            "",
            &count.to_string(),
            "Есть клиенты с одинаковыми ФИО и телефоном; они не объединены.",
        );
    }
}

/// Дата создания клиентов без даты в `ClientsMap`: самая ранняя дата приёма их заказов, иначе время импорта.
pub(crate) fn fill_created_at(stage: &mut ClientStage, orders: &[ImportedOrder]) {
    let mut earliest: HashMap<i64, NaiveDateTime> = HashMap::new();
    for o in orders {
        earliest
            .entry(o.client_id)
            .and_modify(|e| *e = (*e).min(o.received_at))
            .or_insert(o.received_at);
    }
    for client in &mut stage.clients {
        if !stage.dated.contains(&client.id)
            && let Some(at) = earliest.get(&client.id)
        {
            client.created_at = *at;
        }
    }
}
