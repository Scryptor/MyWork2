//! `Catalog` → заказы. Поля-списки (комплектность, состояние, неисправности, выполненные работы)
//! приводятся к виду MyWork 3 «пункт, пункт» (`parse::lists`); исходный текст — в исходной строке
//! заказа (`legacy_rows`).

use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDateTime;
use mw_core::ImportedOrder;
use mw_domain::barcode::{is_valid_upc_a, order_barcode};
use mw_domain::dictionary::DictionaryKind;
use mw_domain::money::Money;
use mw_domain::status::StatusKind;

use crate::convert::catalogs::Catalogs;
use crate::convert::history::HistoryHints;
use crate::parse::lists::{ListField, list_text};
use crate::parse::{
    DateValue, MoneyValue, SUSPICIOUS_RUBLES, extract_tracking, parse_date, parse_flag, parse_money,
};
use crate::report::{WarningCode, Warnings};
use crate::source::{LegacyDb, LegacyTable, LegacyValue};

const SOURCE: &str = "Catalog";

/// Строки `Catalog` с разобранным номером по возрастанию номера.
pub(crate) fn catalog_rows(db: &LegacyDb) -> Vec<(i64, &[LegacyValue])> {
    let Some(table) = db.table(SOURCE) else {
        return Vec::new();
    };
    let mut rows: Vec<_> = table
        .rows
        .iter()
        .filter_map(|r| {
            Some((
                table.text(r, "id").trim().parse::<i64>().ok()?,
                r.as_slice(),
            ))
        })
        .collect();
    rows.sort_by_key(|(id, _)| *id);
    rows.dedup_by_key(|(id, _)| *id);
    rows
}

/// Номера заказов, у которых есть разобранная дата выдачи.
pub(crate) fn issued_ids(db: &LegacyDb) -> BTreeSet<i64> {
    let Some(table) = db.table(SOURCE) else {
        return BTreeSet::new();
    };
    catalog_rows(db)
        .into_iter()
        .filter(|(_, row)| {
            matches!(
                parse_date(&table.text(row, "Data_vidachi")),
                DateValue::Parsed(_) | DateValue::MonthFirst(_)
            )
        })
        .map(|(id, _)| id)
        .collect()
}

fn date_field(w: &mut Warnings, id: i64, field: &str, raw: &str) -> Option<NaiveDateTime> {
    match parse_date(raw) {
        DateValue::Empty => None,
        DateValue::Parsed(d) => Some(d),
        DateValue::MonthFirst(d) => {
            w.push(
                WarningCode::DateMonthFirst,
                SOURCE,
                &id.to_string(),
                field,
                raw,
                "Дата с месяцем больше 12 прочитана как «месяц/день».",
            );
            Some(d)
        }
        DateValue::Invalid => {
            w.push(
                WarningCode::DateUnparsed,
                SOURCE,
                &id.to_string(),
                field,
                raw,
                "Дата не разобрана.",
            );
            None
        }
    }
}

/// `None` — пусто или не разобрано (во втором случае есть предупреждение).
fn money_field(w: &mut Warnings, id: i64, field: &str, raw: &str) -> Option<Money> {
    match parse_money(raw) {
        MoneyValue::Empty => None,
        MoneyValue::Invalid => {
            w.push(
                WarningCode::MoneyUnparsed,
                SOURCE,
                &id.to_string(),
                field,
                raw,
                "Сумма не разобрана.",
            );
            None
        }
        MoneyValue::Parsed(m) => {
            if m.kopecks() > SUSPICIOUS_RUBLES.saturating_mul(100) {
                w.push(
                    WarningCode::AmountSuspicious,
                    SOURCE,
                    &id.to_string(),
                    field,
                    raw,
                    "Сумма больше 1 000 000 ₽ перенесена как есть.",
                );
            }
            Some(m)
        }
    }
}

fn flag_field(w: &mut Warnings, id: i64, field: &str, raw: &str) -> bool {
    parse_flag(raw).unwrap_or_else(|| {
        w.push(
            WarningCode::FlagUnrecognized,
            SOURCE,
            &id.to_string(),
            field,
            raw,
            "Значение флага не распознано, принято как «нет».",
        );
        false
    })
}

/// Дата приёма: своя, иначе подстановка (история, меньший номер, больший номер, время импорта).
fn fill_received(
    w: &mut Warnings,
    ids: &[i64],
    own: &[Option<NaiveDateTime>],
    raws: &[String],
    hints: &HistoryHints,
    now: NaiveDateTime,
) -> Vec<NaiveDateTime> {
    let mut resolved: Vec<NaiveDateTime> = Vec::with_capacity(ids.len());
    for (i, id) in ids.iter().enumerate() {
        if let Some(d) = own[i] {
            resolved.push(d);
            continue;
        }
        let (value, why) = if let Some(d) = hints.earliest.get(id) {
            (*d, "самая ранняя дата из истории заказа")
        } else if let Some(d) = resolved.last() {
            (*d, "дата приёма заказа с меньшим номером")
        } else if let Some(d) = own[i + 1..].iter().flatten().next() {
            (*d, "дата приёма заказа с большим номером")
        } else {
            (now, "время импорта")
        };
        w.push(
            WarningCode::ReceivedAtMissing,
            SOURCE,
            &id.to_string(),
            "Data_priema",
            &raws[i],
            &format!("Нет даты приёма: подставлена {why}."),
        );
        resolved.push(value);
    }
    resolved
}

struct Barcode {
    value: String,
    original: bool,
}

fn barcode_field(w: &mut Warnings, id: i64, raw: &str) -> Barcode {
    let value = raw.trim();
    if value.is_empty() {
        let generated = order_barcode(id).unwrap_or_default();
        w.push(
            WarningCode::BarcodeGenerated,
            SOURCE,
            &id.to_string(),
            "Barcode",
            &generated,
            "Штрихкода не было, создан по номеру заказа.",
        );
        return Barcode {
            value: generated,
            original: false,
        };
    }
    if !is_valid_upc_a(value) {
        w.push(
            WarningCode::BarcodeInvalid,
            SOURCE,
            &id.to_string(),
            "Barcode",
            value,
            "Штрихкод не по формату UPC-A перенесён как есть.",
        );
    }
    Barcode {
        value: value.to_owned(),
        original: true,
    }
}

fn warn_duplicate_barcodes(w: &mut Warnings, found: &[(i64, &Barcode)]) {
    let mut groups: BTreeMap<&str, Vec<i64>> = BTreeMap::new();
    for (id, b) in found.iter().filter(|(_, b)| b.original) {
        groups.entry(&b.value).or_default().push(*id);
    }
    for (code, ids) in groups.into_iter().filter(|(_, ids)| ids.len() > 1) {
        let list: Vec<String> = ids.iter().map(i64::to_string).collect();
        w.push(
            WarningCode::BarcodeDuplicate,
            SOURCE,
            code,
            "Barcode",
            &format!("заказы {}", list.join(", ")),
            "Один штрихкод у нескольких заказов перенесён как есть.",
        );
    }
}

struct Inputs<'a> {
    table: &'a LegacyTable,
    catalogs: &'a Catalogs,
    order_client: &'a BTreeMap<i64, i64>,
    hints: &'a HistoryHints,
    now: NaiveDateTime,
}

fn build_order(
    inp: &Inputs,
    w: &mut Warnings,
    id: i64,
    row: &[LegacyValue],
    received_at: NaiveDateTime,
) -> (ImportedOrder, Barcode) {
    let t = |name: &str| inp.table.text(row, name).trim().to_owned();
    let key = id.to_string();
    let status_raw = inp.table.text(row, "Status_remonta");
    if status_raw.trim().is_empty() {
        w.push(
            WarningCode::StatusEmpty,
            SOURCE,
            &key,
            "Status_remonta",
            "",
            "У заказа нет статуса, назначен «Без статуса».",
        );
    }
    let status = inp.catalogs.status_for(&status_raw);
    let issued_at = date_field(w, id, "Data_vidachi", &inp.table.text(row, "Data_vidachi"));
    let prepaid_at = date_field(
        w,
        id,
        "Data_predoplaty",
        &inp.table.text(row, "Data_predoplaty"),
    );
    let status_issued = inp
        .catalogs
        .statuses
        .iter()
        .any(|s| s.name == status && s.kind == StatusKind::Issued);
    if status_issued && issued_at.is_none() {
        w.push(
            WarningCode::IssuedWithoutDate,
            SOURCE,
            &key,
            "Data_vidachi",
            &inp.table.text(row, "Data_vidachi"),
            "Статус «выдан», а даты выдачи нет.",
        );
    }
    let issued = status_issued || issued_at.is_some();
    let mut money = |field: &str| money_field(w, id, field, &inp.table.text(row, field));
    let estimate = money("predvaritelnaya_stoimost").filter(|m| *m != Money::ZERO);
    let prepayment = money("Predoplata").unwrap_or(Money::ZERO);
    let extra_costs = money("Zatrati").unwrap_or(Money::ZERO);
    let final_text = inp.table.text(row, "okonchatelnaya_stoimost_remonta");
    let final_invalid = matches!(parse_money(&final_text), MoneyValue::Invalid);
    let final_raw = money("okonchatelnaya_stoimost_remonta");
    let discount = money("Skidka").unwrap_or(Money::ZERO);
    let final_price = match final_raw {
        Some(m) if m != Money::ZERO => Some(m),
        _ if final_invalid => None,
        _ if issued => Some(Money::ZERO),
        _ => None,
    };
    let barcode = barcode_field(w, id, &inp.table.text(row, "Barcode"));
    let comment = t("kommentarij");
    let tracking = extract_tracking(&comment);
    if !tracking.is_empty() {
        let numbers: Vec<&str> = tracking.iter().map(|(n, _)| n.as_str()).collect();
        w.push(
            WarningCode::TrackingFound,
            SOURCE,
            &key,
            "kommentarij",
            &numbers.join(", "),
            "В комментарии найдены трек-номера, они перенесены в заказ.",
        );
    }
    let need_call = flag_field(w, id, "Image_key", &inp.table.text(row, "Image_key"));
    let need_parts = flag_field(w, id, "wait_zakaz", &inp.table.text(row, "wait_zakaz"));
    let deleted_raw = inp.table.text(row, "Deleted");
    let deleted = flag_field(w, id, "Deleted", &deleted_raw);
    let deleted_at = deleted.then(|| inp.hints.deleted.get(&id).copied().unwrap_or(inp.now));
    let order = ImportedOrder {
        id,
        client_id: inp.order_client.get(&id).copied().unwrap_or_default(),
        status,
        service_point: inp.catalogs.point_for(&t("AdressSC")),
        received_at,
        issued_at,
        prepaid_at,
        device_type: t("WhatRemont"),
        brand: t("brand"),
        model: t("model"),
        serial: t("SerialNumber"),
        color: t("DeviceColour"),
        completeness: list_text(
            ListField::Completeness,
            &t("komplektonst"),
            inp.catalogs.offered(DictionaryKind::Completeness),
        ),
        device_condition: list_text(ListField::Condition, &t("sostoyanie"), &[]),
        malfunction: list_text(ListField::Malfunction, &t("polomka"), &[]),
        work_done: list_text(ListField::WorkDone, &t("vipolnenie_raboti"), &[]),
        internal_comment: comment,
        master: t("master"),
        barcode: barcode.value.clone(),
        estimate_from: estimate,
        estimate_to: estimate,
        final_price,
        prepayment,
        extra_costs,
        discount,
        warranty: t("Garanty"),
        need_call,
        need_parts,
        deleted_at,
        tracking,
    };
    (order, barcode)
}

/// Заказы по возрастанию номера.
pub(crate) fn convert_orders(
    db: &LegacyDb,
    catalogs: &Catalogs,
    order_client: &BTreeMap<i64, i64>,
    hints: &HistoryHints,
    now: NaiveDateTime,
    w: &mut Warnings,
) -> Vec<ImportedOrder> {
    let Some(table) = db.table(SOURCE) else {
        return Vec::new();
    };
    let rows = catalog_rows(db);
    let ids: Vec<i64> = rows.iter().map(|(id, _)| *id).collect();
    let raws: Vec<String> = rows
        .iter()
        .map(|(_, r)| table.text(r, "Data_priema"))
        .collect();
    let own: Vec<Option<NaiveDateTime>> = rows
        .iter()
        .zip(&raws)
        .map(|((id, _), raw)| date_field(w, *id, "Data_priema", raw))
        .collect();
    let received = fill_received(w, &ids, &own, &raws, hints, now);
    let inp = Inputs {
        table,
        catalogs,
        order_client,
        hints,
        now,
    };
    let built: Vec<(ImportedOrder, Barcode)> = rows
        .iter()
        .zip(&received)
        .map(|((id, row), at)| build_order(&inp, w, *id, row, *at))
        .collect();
    let found: Vec<(i64, &Barcode)> = built.iter().map(|(o, b)| (o.id, b)).collect();
    warn_duplicate_barcodes(w, &found);
    built.into_iter().map(|(o, _)| o).collect()
}
