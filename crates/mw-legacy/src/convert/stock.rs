//! Склад MyWork 2 (решения 13–16 плана 8.1): `Stock` → позиции склада, `StockMap` → запчасти
//! заказов, из «затрат» заказа выделяются запчасти; списки склада — в справочники.
//!
//! - Тексты позиции — как в MyWork 2 (заглавные, двойные пробелы внутри), только без пробелов по
//!   краям; количество и порог — `parse_quantity`, цена — `parse_money` не больше
//!   `MAX_STOCK_PRICE`; не разобранное — ноль и строка отчёта.
//! - Строка `StockMap` — запчасть заказа со снимком выводимого наименования позиции и ценой
//!   списания; позиции нет — запчасть без позиции; заказа нет — запчасти нет.
//! - «Затраты» MyWork 2 включали запчасти: «расходы» заказа уменьшаются на сумму его запчастей,
//!   если не уходят в минус и не менялись после импорта; иначе остаются как есть (в отчёт).

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use mw_core::{ExtraCostsChange, ImportedOrderPart, ImportedStock, ImportedStockItem};
use mw_domain::dictionary::DictionaryKind;
use mw_domain::money::Money;
use mw_domain::search::normalize_search;
use mw_domain::spelling::{has_letter, tidy};
use mw_domain::stock::{MAX_STOCK_PRICE, MAX_STOCK_QUANTITY, StockNameParts, display_name};

use super::catalogs::is_placeholder;
use super::row_key;
use crate::folder::LegacyFolder;
use crate::parse::money::{MoneyValue, parse_money};
use crate::report::{WarningCode, Warnings};
use crate::source::LegacyTable;
use crate::text::{decode_text, text_lines};

/// Папка списков склада среди текстовых файлов папки MyWork 2.
const LISTS_PREFIX: &str = "settings/Stock/";

/// Списки склада и справочники, в которые идут их строки (типы запчастей — ещё и типы позиций).
const LIST_FILES: [(DictionaryKind, &str); 4] = [
    (DictionaryKind::PartType, "TypeOvZIP.txt"),
    (DictionaryKind::DeviceType, "ustrojstvo.txt"),
    (DictionaryKind::Brand, "brands.txt"),
    (DictionaryKind::Color, "DeviceColour.txt"),
];

/// «Расходы» заказа: сейчас и как их записал импорт (`Zatrati`). Правятся, только если равны.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OrderCosts {
    pub current: Money,
    pub imported: Money,
}

/// Что читает перенос склада: таблицы, тексты списков (имя файла `settings/Stock/` → текст),
/// «расходы» заказов (номер → расходы) и счётчик `sqlite_sequence` таблицы `Stock` (нет — 0).
pub(crate) struct StockSource<'a> {
    pub stock: Option<&'a LegacyTable>,
    pub stock_map: Option<&'a LegacyTable>,
    pub lists: &'a BTreeMap<String, String>,
    pub orders: &'a BTreeMap<i64, OrderCosts>,
    pub sequence: i64,
}

/// Перенесённый склад: позиции и запчасти, правки «расходов», добавки в справочники (виды без
/// добавок не перечисляются), номера строк `Stock`, ставших позициями, и ключи строк `StockMap`,
/// ставших запчастями (→ номер заказа).
pub(crate) struct StockConversion {
    pub stock: ImportedStock,
    pub cost_changes: Vec<ExtraCostsChange>,
    pub dictionary_additions: Vec<(DictionaryKind, Vec<String>)>,
    pub stock_targets: Vec<i64>,
    pub part_targets: Vec<(String, i64)>,
}

/// Тексты списков склада (`settings/Stock/*.txt`): имя файла → текст. Строк отчёта о кодировке
/// здесь нет: `TypeOvZIP.txt` читает и называет справочник типов запчастей (`super::catalogs`),
/// остальные списки склада только дополняют справочники.
pub(crate) fn stock_lists(folder: &LegacyFolder) -> BTreeMap<String, String> {
    folder
        .text_files
        .iter()
        .filter_map(|(path, bytes)| Some((list_name(path)?.to_owned(), decode_text(bytes).0)))
        .collect()
}

/// Имя списка склада по пути файла папки MyWork 2 (он же ключ следа `file`); не список — пусто.
pub(crate) fn list_name(path: &str) -> Option<&str> {
    path.strip_prefix(LISTS_PREFIX)
}

/// Количество MyWork 2 (`decimal` текстом): цифры, допускаются хвосты «.0…» и «,0…»; не больше
/// `MAX_STOCK_QUANTITY`. Пусто — ноль.
pub(crate) fn parse_quantity(raw: &str) -> Option<i64> {
    let text = raw.trim();
    if text.is_empty() {
        return Some(0);
    }
    let (whole, fraction) = match text.find(['.', ',']) {
        Some(at) => (&text[..at], Some(&text[at + 1..])),
        None => (text, None),
    };
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let zeros = |s: &str| digits(s) && s.bytes().all(|b| b == b'0');
    if !digits(whole) || fraction.is_some_and(|f| !zeros(f)) {
        return None;
    }
    whole
        .parse::<i64>()
        .ok()
        .filter(|n| *n <= MAX_STOCK_QUANTITY)
}

/// Одна ячейка склада: где она и что в ней.
struct Cell<'a> {
    source: &'a str,
    key: &'a str,
    field: &'a str,
    raw: String,
}

fn quantity(w: &mut Warnings, cell: &Cell<'_>) -> i64 {
    parse_quantity(&cell.raw).unwrap_or_else(|| {
        w.push(
            WarningCode::QuantityUnparsed,
            cell.source,
            cell.key,
            cell.field,
            &cell.raw,
            "Количество не разобрано: записан ноль.",
        );
        0
    })
}

fn price(w: &mut Warnings, cell: &Cell<'_>) -> Money {
    let message = match parse_money(&cell.raw) {
        MoneyValue::Empty => return Money::ZERO,
        MoneyValue::Parsed(m) if m.kopecks() <= MAX_STOCK_PRICE.kopecks() => return m,
        MoneyValue::Parsed(_) => {
            "Цена больше 10 000 000 ₽ — больше, чем бывает на складе: записан ноль."
        }
        MoneyValue::Invalid => "Сумма не разобрана: записан ноль.",
    };
    w.push(
        WarningCode::MoneyUnparsed,
        cell.source,
        cell.key,
        cell.field,
        &cell.raw,
        message,
    );
    Money::ZERO
}

/// Позиции склада из строк `Stock`: номер — целое больше нуля и не повтор, иначе строка не
/// переносится (в отчёт).
fn convert_items(table: &LegacyTable, w: &mut Warnings) -> Vec<ImportedStockItem> {
    let mut items = Vec::new();
    let mut taken = BTreeSet::new();
    for (n, row) in table.rows.iter().enumerate() {
        let key = row_key(table, row, n);
        let raw_id = table.text(row, "id");
        let id = raw_id
            .trim()
            .parse::<i64>()
            .ok()
            .filter(|id| *id > 0 && taken.insert(*id));
        let Some(id) = id else {
            w.push(
                WarningCode::StockRowSkipped,
                "Stock",
                &key,
                "id",
                &raw_id,
                "Номер позиции — не целое число больше нуля или повтор: позиция не создана, строка сохранена в исходном виде.",
            );
            continue;
        };
        let text = |column: &str| table.text(row, column).trim().to_owned();
        let cell = |field| Cell {
            source: "Stock",
            key: &key,
            field,
            raw: table.text(row, field),
        };
        items.push(ImportedStockItem {
            id,
            name: text("Naimenovanie"),
            device_type: text("Kategoriya"),
            part_type: text("Podkategoriya"),
            brand: text("Brand"),
            model: text("Model"),
            color: text("Colour"),
            note: text("Primechanie"),
            quantity: quantity(w, &cell("CountOf")),
            price: price(w, &cell("Price")),
            restock_threshold: quantity(w, &cell("Napominanie")),
        });
    }
    items
}

/// Выводимое наименование позиции — как у позиции MyWork 3 (`mw_core` `display_name`).
fn item_name(item: &ImportedStockItem) -> String {
    display_name(
        &item.name,
        &StockNameParts {
            part_type: &item.part_type,
            device_type: &item.device_type,
            brand: &item.brand,
            model: &item.model,
            color: &item.color,
        },
        item.id,
    )
}

/// Запчасти заказов из строк `StockMap` и ключи строк, ставших запчастями (→ номер заказа).
fn convert_parts(
    table: &LegacyTable,
    items: &[ImportedStockItem],
    orders: &BTreeMap<i64, OrderCosts>,
    w: &mut Warnings,
) -> (Vec<ImportedOrderPart>, Vec<(String, i64)>) {
    let names: HashMap<i64, String> = items.iter().map(|i| (i.id, item_name(i))).collect();
    let mut parts = Vec::new();
    let mut targets = Vec::new();
    for (n, row) in table.rows.iter().enumerate() {
        let key = row_key(table, row, n);
        let raw_order = table.text(row, "clientId");
        let order_id = order_number(&raw_order).filter(|id| orders.contains_key(id));
        let Some(order_id) = order_id else {
            w.push(
                WarningCode::StockPartWithoutOrder,
                "StockMap",
                &key,
                "clientId",
                &raw_order,
                "Заказа с таким номером нет: запчасть не создана, строка сохранена в исходном виде.",
            );
            continue;
        };
        let raw_item = table.text(row, "ZIPId");
        let item = raw_item
            .trim()
            .parse::<i64>()
            .ok()
            .and_then(|id| names.get_key_value(&id));
        let (stock_item_id, name) = match item {
            Some((id, name)) => (Some(*id), name.clone()),
            None => {
                w.push(
                    WarningCode::StockPartOrphan,
                    "StockMap",
                    &key,
                    "ZIPId",
                    &raw_item,
                    "Позиции склада с таким номером нет: запчасть перенесена без позиции.",
                );
                let shown = raw_item.trim();
                let name = if shown.is_empty() {
                    "Позиция (нет на складе)".to_owned()
                } else {
                    format!("Позиция №{shown} (нет на складе)")
                };
                (None, name)
            }
        };
        let cell = |field| Cell {
            source: "StockMap",
            key: &key,
            field,
            raw: table.text(row, field),
        };
        parts.push(ImportedOrderPart {
            order_id,
            stock_item_id,
            name,
            price: price(w, &cell("priceOfZIP")),
            quantity: quantity(w, &cell("countOfZIP")),
        });
        targets.push((key, order_id));
    }
    (parts, targets)
}

/// Номер заказа строки `StockMap` по тексту её `clientId` (`LegacyTable::text`: NUL уже убран).
pub(crate) fn order_number(client_id: &str) -> Option<i64> {
    client_id.trim().parse().ok()
}

/// Сумма для отчёта: «1 000,00» (простые пробелы, всегда копейки).
fn amount_text(kopecks: i128) -> String {
    let sign = if kopecks < 0 { "-" } else { "" };
    let abs = kopecks.unsigned_abs();
    let rubles = (abs / 100).to_string();
    let mut grouped = String::new();
    for (i, ch) in rubles.chars().enumerate() {
        if i > 0 && (rubles.len() - i).is_multiple_of(3) {
            grouped.push(' ');
        }
        grouped.push(ch);
    }
    format!("{sign}{grouped},{:02}", abs % 100)
}

/// «Расходы» без запчастей: по заказам с запчастями на ненулевую сумму.
fn split_costs(
    parts: &[ImportedOrderPart],
    orders: &BTreeMap<i64, OrderCosts>,
    w: &mut Warnings,
) -> Vec<ExtraCostsChange> {
    // Цена × количество, как `part_sum`; сумма заказа — в i128: переполнения нет.
    let mut sums: BTreeMap<i64, i128> = BTreeMap::new();
    for p in parts {
        *sums.entry(p.order_id).or_default() +=
            i128::from(p.price.kopecks()) * i128::from(p.quantity);
    }
    let mut changes = Vec::new();
    for (order_id, sum) in sums {
        let Some(costs) = orders.get(&order_id) else {
            continue;
        };
        if sum == 0 {
            continue;
        }
        let current = i128::from(costs.current.kopecks());
        let message = if costs.current != costs.imported {
            "Расходы заказа менялись после импорта: оставлены как есть, запчасти из них не вычтены."
        } else if current >= sum {
            changes.push(ExtraCostsChange {
                order_id,
                from: costs.current,
                to: Money::from_kopecks(i64::try_from(current - sum).unwrap_or(0)),
            });
            continue;
        } else {
            "Затраты MyWork 2 меньше суммы запчастей заказа: оставлены как есть, запчасти из них не вычтены."
        };
        w.push(
            WarningCode::ExtraCostsKept,
            "Catalog",
            &order_id.to_string(),
            "Zatrati",
            &format!(
                "затраты {}, запчасти {}",
                amount_text(current),
                amount_text(sum)
            ),
            message,
        );
    }
    changes
}

/// Значения склада для справочников, которых там нет (без учёта регистра): типы запчастей — из
/// `TypeOvZIP.txt`, затем типы позиций; типы устройств, бренды, цвета — из списков склада.
fn dictionary_additions(
    lists: &BTreeMap<String, String>,
    items: &[ImportedStockItem],
    existing: &[(DictionaryKind, Vec<String>)],
) -> Vec<(DictionaryKind, Vec<String>)> {
    LIST_FILES
        .iter()
        .filter_map(|(kind, file)| {
            let mut candidates = lists.get(*file).map(|t| text_lines(t)).unwrap_or_default();
            if *kind == DictionaryKind::PartType {
                candidates.extend(items.iter().map(|i| i.part_type.clone()));
            }
            let mut known: HashSet<String> = existing
                .iter()
                .filter(|(k, _)| k == kind)
                .flat_map(|(_, values)| values.iter().map(|v| normalize_search(v)))
                .collect();
            let added: Vec<String> = candidates
                .iter()
                .map(|v| tidy(v))
                .filter(|v| {
                    has_letter(v) && !is_placeholder(v) && known.insert(normalize_search(v))
                })
                .collect();
            (!added.is_empty()).then_some((*kind, added))
        })
        .collect()
}

/// Переносит склад; `existing` — справочники, к которым добавляются значения склада.
pub(crate) fn convert_stock(
    src: &StockSource<'_>,
    existing: &[(DictionaryKind, Vec<String>)],
    w: &mut Warnings,
) -> StockConversion {
    let items = src.stock.map(|t| convert_items(t, w)).unwrap_or_default();
    let (parts, part_targets) = src
        .stock_map
        .map(|t| convert_parts(t, &items, src.orders, w))
        .unwrap_or_default();
    let cost_changes = split_costs(&parts, src.orders, w);
    let dictionary_additions = dictionary_additions(src.lists, &items, existing);
    let had_rows = [src.stock, src.stock_map]
        .iter()
        .any(|t| t.is_some_and(|t| !t.rows.is_empty()));
    if had_rows {
        w.push(
            WarningCode::StockImported,
            "Stock",
            "",
            "",
            &format!("позиций {}, запчастей {}", items.len(), parts.len()),
            "Позиции перенесены на склад, списания — запчастями заказов.",
        );
    }
    let last = items.iter().map(|i| i.id).max().unwrap_or(0);
    StockConversion {
        stock_targets: items.iter().map(|i| i.id).collect(),
        stock: ImportedStock {
            next_stock_item_id: src.sequence.max(last).saturating_add(1).max(1),
            items,
            parts,
        },
        cost_changes,
        dictionary_additions,
        part_targets,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mw_core::{ExtraCostsChange, ImportedOrderPart, ImportedStockItem, LegacyTarget};
    use mw_domain::dictionary::{DEFAULT_DICTIONARY, DictionaryKind};
    use mw_domain::money::Money;

    use super::*;
    use crate::convert::Conversion;
    use crate::fixture::FixtureVersion::V32;
    use crate::parse::money::{MoneyValue, parse_money};
    use crate::report::{Warning, Warnings};
    use crate::source::LegacyDb;
    use crate::test_support::{conversion_of, conversion_with_sql, order, prepared, rub};

    fn kop(kopecks: i64) -> Money {
        Money::from_kopecks(kopecks)
    }

    fn item(c: &Conversion, id: i64) -> &ImportedStockItem {
        c.bundle.stock.items.iter().find(|i| i.id == id).unwrap()
    }

    /// Предупреждения кода: (источник, ключ, поле, значение).
    fn warned<'a>(ws: &'a [Warning], code: &str) -> Vec<(&'a str, &'a str, &'a str, &'a str)> {
        ws.iter()
            .filter(|w| w.code.as_str() == code)
            .map(|w| {
                (
                    w.source.as_str(),
                    w.key.as_str(),
                    w.field.as_str(),
                    w.value.as_str(),
                )
            })
            .collect()
    }

    fn target(c: &Conversion, source: &str, key: &str) -> Option<LegacyTarget> {
        c.bundle
            .legacy_rows
            .iter()
            .find(|r| r.source == source && r.source_key == key)
            .unwrap_or_else(|| panic!("нет следа {source}/{key}"))
            .target
            .clone()
    }

    /// «Затраты» заказов, как их разобрал бы импорт заказов (`Zatrati`; не разобрано — ноль).
    fn legacy_costs(db: &LegacyDb) -> BTreeMap<i64, OrderCosts> {
        let t = db.table("Catalog").unwrap();
        t.rows
            .iter()
            .map(|row| {
                let id: i64 = t.text(row, "id").parse().unwrap();
                let costs = match parse_money(&t.text(row, "Zatrati")) {
                    MoneyValue::Parsed(m) => m,
                    _ => Money::ZERO,
                };
                (
                    id,
                    OrderCosts {
                        current: costs,
                        imported: costs,
                    },
                )
            })
            .collect()
    }

    /// Перенос склада фикстуры отдельно от остального: со справочниками MyWork 3 по умолчанию.
    async fn stock_of(
        edit: impl FnOnce(&mut BTreeMap<i64, OrderCosts>),
    ) -> (StockConversion, Warnings) {
        let (db, folder) = prepared(V32).await;
        let mut costs = legacy_costs(&db);
        edit(&mut costs);
        let lists = super::stock_lists(&folder);
        let existing: Vec<(DictionaryKind, Vec<String>)> = DEFAULT_DICTIONARY
            .iter()
            .map(|(k, v)| (*k, v.iter().map(|s| (*s).to_owned()).collect()))
            .collect();
        let mut w = Warnings::default();
        let src = StockSource {
            stock: db.table("Stock"),
            stock_map: db.table("StockMap"),
            lists: &lists,
            orders: &costs,
            sequence: 0,
        };
        let out = convert_stock(&src, &existing, &mut w);
        (out, w)
    }

    /// Тексты — как в MyWork 2 (заглавные, двойной пробел внутри), только без пробелов по краям;
    /// количество, порог и цена разобраны, неразобранное — ноль и строка отчёта.
    #[tokio::test]
    async fn items_keep_legacy_text() {
        let c = conversion_of(V32).await;
        assert_eq!(
            *item(&c, 3),
            ImportedStockItem {
                id: 3,
                name: "ТЕЛЕФОН АККУМУЛЯТОР XIAOMI REDMI 9  (ОРИГИНАЛ)".into(),
                device_type: "ТЕЛЕФОН".into(),
                part_type: "АККУМУЛЯТОР".into(),
                brand: "XIAOMI".into(),
                model: "REDMI 9".into(),
                color: String::new(),
                note: "ОРИГИНАЛ".into(),
                quantity: 2,
                price: kop(120_050),
                restock_threshold: 5,
            }
        );
        assert_eq!(
            *item(&c, 1),
            ImportedStockItem {
                id: 1,
                name: "Дисплей".into(),
                device_type: "Запчасти".into(),
                part_type: "Дисплеи".into(),
                brand: "SAMSUNG".into(),
                model: "A52".into(),
                color: "Чёрный".into(),
                note: String::new(),
                quantity: 3,
                price: rub(2500),
                restock_threshold: 1,
            }
        );
        // «два» — ноль и строка отчёта; пустые цена и порог — ноль без строки.
        let two = item(&c, 2);
        assert_eq!(
            (two.quantity, two.price, two.restock_threshold),
            (0, Money::ZERO, 0)
        );
        // «3.0» и «3,00» — 3; цена «abc» — ноль и строка отчёта.
        let four = item(&c, 4);
        assert_eq!(
            (four.quantity, four.price, four.restock_threshold),
            (3, Money::ZERO, 3)
        );
        let ws = c.warnings.list();
        assert_eq!(
            warned(ws, "quantity_unparsed")
                .into_iter()
                .filter(|w| w.0 == "Stock")
                .collect::<Vec<_>>(),
            [("Stock", "2", "CountOf", "два")]
        );
        assert_eq!(
            warned(ws, "money_unparsed")
                .into_iter()
                .filter(|w| w.0 == "Stock")
                .collect::<Vec<_>>(),
            [("Stock", "4", "Price", "abc")]
        );
        assert_eq!(
            c.bundle
                .stock
                .items
                .iter()
                .map(|i| i.id)
                .collect::<Vec<_>>(),
            [1, 2, 3, 4]
        );
        assert_eq!(c.bundle.stock.next_stock_item_id, 5);
        // След строки — позиция, фото — позиция; склада «в исходном виде» больше нет.
        assert_eq!(target(&c, "Stock", "3"), Some(LegacyTarget::StockItem(3)));
        let photos: Vec<_> = c
            .files
            .iter()
            .filter(|f| f.legacy_source == "Stock.Photo")
            .map(|f| (f.legacy_key.as_str(), f.target.clone()))
            .collect();
        assert_eq!(
            photos,
            [
                ("1/Photo", Some(LegacyTarget::StockItem(1))),
                ("1/Photo2", Some(LegacyTarget::StockItem(1))),
                ("3/Photo", Some(LegacyTarget::StockItem(3))),
                ("3/Photo2", Some(LegacyTarget::StockItem(3))),
                ("3/Photo3", Some(LegacyTarget::StockItem(3))),
            ]
        );
        assert!(warned(ws, "stock_preserved").is_empty());
        assert_eq!(
            warned(ws, "stock_imported"),
            [("Stock", "", "", "позиций 4, запчастей 5")]
        );
    }

    #[test]
    fn parse_quantity_takes_whole_numbers_of_decimal_text() {
        for (raw, expected) in [
            ("3", Some(3)),
            (" 3,00 ", Some(3)),
            ("3.0", Some(3)),
            ("0", Some(0)),
            ("1000000", Some(1_000_000)),
            ("", Some(0)),
            ("  ", Some(0)),
            ("-1", None),
            ("2.5", None),
            ("3,", None),
            ("два", None),
            ("1000001", None),
            ("99999999999999999999999", None),
        ] {
            assert_eq!(parse_quantity(raw), expected, "{raw:?}");
        }
    }

    /// Строка `StockMap` — запчасть заказа со снимком наименования позиции и цены списания;
    /// позиции нет — запчасть без позиции; заказа нет — запчасти нет.
    #[tokio::test]
    async fn parts_from_stock_map() {
        let c = conversion_of(V32).await;
        let part = |order_id, stock_item_id, name: &str, price, quantity| ImportedOrderPart {
            order_id,
            stock_item_id,
            name: name.to_owned(),
            price,
            quantity,
        };
        assert_eq!(
            c.bundle.stock.parts,
            [
                part(2, Some(1), "Дисплей", rub(2500), 1),
                part(
                    2,
                    Some(3),
                    "ТЕЛЕФОН АККУМУЛЯТОР XIAOMI REDMI 9  (ОРИГИНАЛ)",
                    kop(120_050),
                    1
                ),
                part(5, None, "Позиция №99 (нет на складе)", rub(300), 1),
                part(3, Some(2), "Аккумулятор", rub(800), 0),
                part(4, Some(1), "Дисплей", rub(2500), 2),
            ]
        );
        let ws = c.warnings.list();
        assert_eq!(
            warned(ws, "stock_part_orphan"),
            [("StockMap", "3", "ZIPId", "99")]
        );
        assert_eq!(
            warned(ws, "stock_part_without_order"),
            [("StockMap", "4", "clientId", "404")]
        );
        assert!(warned(ws, "quantity_unparsed").contains(&("StockMap", "5", "countOfZIP", "два")));
        // Следы `StockMap` — у заказа запчасти; без заказа — без цели.
        assert_eq!(target(&c, "StockMap", "1"), Some(LegacyTarget::Order(2)));
        assert_eq!(target(&c, "StockMap", "3"), Some(LegacyTarget::Order(5)));
        assert_eq!(target(&c, "StockMap", "4"), None);
    }

    /// «Затраты» MyWork 2 = ручные расходы + запчасти склада: расходы — без запчастей, если не уходят
    /// в минус; иначе остаются как есть со строкой отчёта.
    #[tokio::test]
    async fn extra_costs_are_split() {
        let (s, w) = stock_of(|_| {}).await;
        assert_eq!(
            s.cost_changes,
            [
                ExtraCostsChange {
                    order_id: 2,
                    from: rub(4000),
                    to: kop(29_950),
                },
                ExtraCostsChange {
                    order_id: 5,
                    from: rub(500),
                    to: rub(200),
                },
            ]
        );
        assert_eq!(
            warned(w.list(), "extra_costs_kept"),
            [(
                "Catalog",
                "4",
                "Zatrati",
                "затраты 1 000,00, запчасти 5 000,00"
            )]
        );
        // В пакете — расходы без запчастей.
        let c = conversion_of(V32).await;
        let costs = |id| order(&c.bundle, id).extra_costs;
        assert_eq!(
            (costs(2), costs(3), costs(4), costs(5)),
            (kop(29_950), Money::ZERO, rub(1000), rub(200))
        );
    }

    /// Расходы, изменённые после импорта (перенос склада в уже импортированную базу), не трогаются.
    #[tokio::test]
    async fn costs_changed_after_the_import_are_kept() {
        let (s, w) = stock_of(|costs| {
            costs.get_mut(&2).unwrap().current = rub(5000);
        })
        .await;
        assert_eq!(
            s.cost_changes
                .iter()
                .map(|c| c.order_id)
                .collect::<Vec<_>>(),
            [5]
        );
        let kept: Vec<_> = w
            .list()
            .iter()
            .filter(|x| x.code.as_str() == "extra_costs_kept")
            .map(|x| {
                (
                    x.key.as_str(),
                    x.value.as_str(),
                    x.message.contains("менялись после импорта"),
                )
            })
            .collect();
        assert_eq!(
            kept,
            [
                ("2", "затраты 5 000,00, запчасти 3 700,50", true),
                ("4", "затраты 1 000,00, запчасти 5 000,00", false),
            ]
        );
        assert_eq!(
            s.part_targets,
            [
                ("1".to_owned(), 2),
                ("2".to_owned(), 2),
                ("3".to_owned(), 5),
                ("5".to_owned(), 3),
                ("6".to_owned(), 4),
            ]
        );
        assert_eq!(s.stock_targets, [1, 2, 3, 4]);
    }

    fn dictionary(c: &Conversion, kind: DictionaryKind) -> Vec<&str> {
        c.bundle
            .dictionary
            .iter()
            .find(|(k, _)| *k == kind)
            .map(|(_, values)| values.iter().map(String::as_str).collect())
            .unwrap()
    }

    fn standard_with(kind: DictionaryKind, extra: &[&'static str]) -> Vec<&'static str> {
        let mut values: Vec<&str> = DEFAULT_DICTIONARY
            .iter()
            .find(|(k, _)| *k == kind)
            .map(|(_, v)| v.to_vec())
            .unwrap();
        values.extend(extra);
        values
    }

    /// Типы запчастей — из `TypeOvZIP.txt`, затем типы позиций; списки склада — в конец типов
    /// устройств, брендов и цветов, если таких значений там нет (без учёта регистра).
    #[tokio::test]
    async fn stock_dictionaries() {
        use DictionaryKind::*;
        let c = conversion_of(V32).await;
        assert_eq!(
            dictionary(&c, PartType),
            ["АККУМУЛЯТОР", "ДИСПЛЕЙ", "Дисплеи"]
        );
        // «ПЛАНШЕТ» и «ЗОЛОТОЙ» уже есть («Планшет», «Золотой»).
        assert_eq!(
            dictionary(&c, DeviceType),
            standard_with(DeviceType, &["ЭЛЕКТРОСАМОКАТ"])
        );
        assert_eq!(dictionary(&c, Color), standard_with(Color, &["БРОНЗОВЫЙ"]));
        // «SAMSUNG» из `Stock/brands.txt` — уже есть «Samsung».
        assert_eq!(dictionary(&c, Brand), ["Nokia", "Samsung", "LG", "Apple"]);
        let lines: BTreeMap<&str, &str> = c
            .warnings
            .list()
            .iter()
            .filter(|w| w.code.as_str() == "dictionary_from_orders")
            .map(|w| (w.key.as_str(), w.value.as_str()))
            .collect();
        assert_eq!(lines["Типы запчастей"], "из файла — 2, со склада — 1");
        assert_eq!(
            lines["Типы устройств"],
            "из заказов — 0, стандартных — 8, значений из одного заказа — 10, со склада — 1"
        );
        assert_eq!(
            lines["Бренды"],
            "из заказов — 3, из файла — 1, значений из одного заказа — 2"
        );
        // Отдельно от импорта — к стандартным справочникам: «АККУМУЛЯТОР» и «ДИСПЛЕЙ» файла там
        // уже есть, «SAMSUNG» тоже; виды без добавок не перечисляются.
        let (s, _) = stock_of(|_| {}).await;
        let owned = |values: &[&str]| values.iter().map(|v| (*v).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            s.dictionary_additions,
            vec![
                (PartType, owned(&["Дисплеи"])),
                (DeviceType, owned(&["ЭЛЕКТРОСАМОКАТ"])),
                (Color, owned(&["БРОНЗОВЫЙ"])),
            ]
        );
    }

    /// Номер позиции не больше нуля — строка не переносится; цена больше предела позиции склада
    /// (10 000 000 ₽) — ноль и строка отчёта, как не разобранная: сумма запчастей не переполнится.
    #[tokio::test]
    async fn bad_numbers_and_prices_over_the_limit() {
        let c = conversion_with_sql(
            V32,
            &[
                "UPDATE Stock SET id = -2 WHERE id = 2",
                "UPDATE Stock SET Price = '10000000,01' WHERE id = 3",
                "UPDATE StockMap SET priceOfZIP = '20 000 000' WHERE id = 6",
            ],
        )
        .await;
        assert_eq!(
            c.bundle
                .stock
                .items
                .iter()
                .map(|i| i.id)
                .collect::<Vec<_>>(),
            [1, 3, 4]
        );
        let ws = c.warnings.list();
        assert_eq!(
            warned(ws, "stock_row_skipped"),
            [("Stock", "-2", "id", "-2")]
        );
        assert_eq!(target(&c, "Stock", "-2"), None);
        assert_eq!(item(&c, 3).price, Money::ZERO);
        let money: Vec<_> = warned(ws, "money_unparsed")
            .into_iter()
            .filter(|w| w.0 != "Catalog")
            .collect();
        assert_eq!(
            money,
            [
                ("Stock", "3", "Price", "10000000,01"),
                ("Stock", "4", "Price", "abc"),
                ("StockMap", "6", "priceOfZIP", "20 000 000"),
            ]
        );
        // Позиции 2 больше нет: её списание — без позиции.
        assert!(warned(ws, "stock_part_orphan").contains(&("StockMap", "5", "ZIPId", "2")));
        let p6 = c.bundle.stock.parts.last().unwrap();
        assert_eq!((p6.order_id, p6.price, p6.quantity), (4, Money::ZERO, 2));
    }
}
