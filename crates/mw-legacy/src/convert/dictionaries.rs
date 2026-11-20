//! Справочники MyWork 3 при переносе: значения, которые писали в заказах (и у клиентов — «Откуда
//! узнали»), затем значения списков `settings/*.txt`, которых в заказах нет (файла нет или он не
//! менялся — стандартные значения MyWork 3; `super::catalogs`).
//!
//! Поля — те же, что у счёта использования (`usage_target` в `mw-core/src/usage.rs`); поля-списки
//! делятся так же (`mw_domain::dictionary::list_items`). Удалённые заказы не считаются, как и там.
//! - Короткие поля (тип устройства, бренд, цвет, гарантия, «Откуда узнали») — каждое значение;
//!   поля-списки — пункт хотя бы из двух заказов (одноразовые фразы в справочник не идут).
//! - Значения сводятся без учёта регистра (`normalize_search`); текст без букв («-», «?», «0») —
//!   не значение, кроме гарантии со сроком («90» — 90 дней).
//! - Написание — из списка, если там есть такое значение, иначе самое частое в заказах
//!   (`mw_domain::spelling::Spellings`), края обрезаны, повторные пробелы схлопнуты.
//! - Порядок — по убыванию числа заказов (клиентов), при равенстве — по первому появлению (по
//!   номерам); затем значения списка, которых нет среди взятых из заказов, в порядке списка.

use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};

use mw_core::{ImportedClient, ImportedOrder};
use mw_domain::dictionary::{DictionaryKind, list_items};
use mw_domain::search::normalize_search;
use mw_domain::spelling::{Spellings, has_letter, tidy};
use mw_domain::warranty::parse_warranty_term;

use super::catalogs::ListedValues;
use crate::report::{WarningCode, Warnings};

/// Одно значение в заказах: ключ, написания, сколько заказов (клиентов).
struct Counted {
    key: String,
    spellings: Spellings,
    records: u64,
}

/// Значения одного вида справочника в порядке первого появления.
#[derive(Default)]
struct Tally {
    positions: HashMap<String, usize>,
    values: Vec<Counted>,
}

impl Tally {
    /// Одна запись (заказ, клиент) со своими значениями: повтор значения в записи — та же запись.
    fn add_record(&mut self, items: impl IntoIterator<Item = String>) {
        let mut seen = HashSet::new();
        for item in items {
            let key = normalize_search(&item);
            if !seen.insert(key.clone()) {
                continue;
            }
            let i = match self.positions.get(&key) {
                Some(i) => *i,
                None => {
                    self.positions.insert(key.clone(), self.values.len());
                    self.values.push(Counted {
                        key,
                        spellings: Spellings::default(),
                        records: 0,
                    });
                    self.values.len() - 1
                }
            };
            self.values[i].records += 1;
            self.values[i].spellings.add(&item, 1);
        }
    }
}

/// Значение ли это: есть буква; у гарантии — ещё и срок из одних цифр («90»).
fn is_value(kind: DictionaryKind, text: &str) -> bool {
    has_letter(text)
        || (kind == DictionaryKind::Warranty
            && parse_warranty_term(text).is_some_and(|term| term.value > 0))
}

/// Тексты вида справочника по записям MyWork 3 (как `usage_target`) и поле-список ли это;
/// `None` — мастера (они пользователи).
fn texts<'a>(
    kind: DictionaryKind,
    orders: &[&'a ImportedOrder],
    clients: &[&'a ImportedClient],
) -> Option<(Vec<&'a str>, bool)> {
    let of = |field: fn(&'a ImportedOrder) -> &'a str| orders.iter().map(|o| field(o)).collect();
    Some(match kind {
        DictionaryKind::DeviceType => (of(|o| &o.device_type), false),
        DictionaryKind::Brand => (of(|o| &o.brand), false),
        DictionaryKind::Color => (of(|o| &o.color), false),
        DictionaryKind::Warranty => (of(|o| &o.warranty), false),
        DictionaryKind::Completeness => (of(|o| &o.completeness), true),
        DictionaryKind::Condition => (of(|o| &o.device_condition), true),
        DictionaryKind::Malfunction => (of(|o| &o.malfunction), true),
        DictionaryKind::WorkDone => (of(|o| &o.work_done), true),
        DictionaryKind::ClientSource => {
            (clients.iter().map(|c| c.source.as_str()).collect(), false)
        }
        DictionaryKind::Master => return None,
    })
}

/// Значения справочника: из заказов, затем только из списка; и сколько тех и других.
fn merge(
    kind: DictionaryKind,
    texts: &[&str],
    list: bool,
    listed: &ListedValues,
) -> (Vec<String>, usize, usize) {
    let mut tally = Tally::default();
    for text in texts {
        let items: Vec<String> = if list {
            list_items(text).map(tidy).collect()
        } else {
            vec![tidy(text)]
        };
        tally.add_record(items.into_iter().filter(|item| is_value(kind, item)));
    }
    let least = if list { 2 } else { 1 };
    let mut used: Vec<&Counted> = tally.values.iter().filter(|c| c.records >= least).collect();
    // Сортировка устойчивая: при равном числе — по первому появлению.
    used.sort_by_key(|c| Reverse(c.records));

    let mut by_key: HashMap<String, &str> = HashMap::new();
    for value in &listed.values {
        by_key.entry(normalize_search(value)).or_insert(value);
    }
    let mut taken = HashSet::new();
    let mut values = Vec::new();
    for c in used {
        let spelling = by_key
            .get(&c.key)
            .copied()
            .or_else(|| c.spellings.best())
            .unwrap_or_default();
        taken.insert(c.key.clone());
        values.push(spelling.to_owned());
    }
    let from_orders = values.len();
    for value in &listed.values {
        if taken.insert(normalize_search(value)) {
            values.push(value.clone());
        }
    }
    let from_list = values.len() - from_orders;
    (values, from_orders, from_list)
}

/// Справочники пакета импорта: каждый вид из `listed` ровно один раз (мастера — пустой список) и
/// строка отчёта на вид — сколько значений из заказов и сколько из списка.
pub(crate) fn build_dictionaries(
    listed: &[(DictionaryKind, ListedValues)],
    orders: &[ImportedOrder],
    clients: &[ImportedClient],
    w: &mut Warnings,
) -> Vec<(DictionaryKind, Vec<String>)> {
    let mut live: Vec<&ImportedOrder> = orders.iter().filter(|o| o.deleted_at.is_none()).collect();
    live.sort_by_key(|o| o.id);
    let mut clients: Vec<&ImportedClient> = clients.iter().collect();
    clients.sort_by_key(|c| c.id);
    listed
        .iter()
        .map(|(kind, list_values)| {
            let Some((texts, list)) = texts(*kind, &live, &clients) else {
                return (*kind, Vec::new());
            };
            let (values, from_orders, from_list) = merge(*kind, &texts, list, list_values);
            let clients = *kind == DictionaryKind::ClientSource;
            let (records, first) = match (clients, list) {
                (true, _) => (
                    "из клиентов",
                    "Сначала все значения из карточек клиентов — по числу клиентов",
                ),
                (false, false) => (
                    "из заказов",
                    "Сначала все значения из заказов — по числу заказов",
                ),
                (false, true) => (
                    "из заказов",
                    "Сначала пункты, которые есть хотя бы в двух заказах, — по числу заказов",
                ),
            };
            let (rest, rest_text) = if list_values.standard {
                ("стандартных", "стандартные значения MyWork 3")
            } else {
                ("из файла", "значения файла MyWork 2")
            };
            w.push(
                WarningCode::DictionaryFromOrders,
                "dictionary",
                kind.label_ru(),
                "",
                &format!("{records} — {from_orders}, {rest} — {from_list}"),
                &format!("{first}, затем {rest_text}, которых среди них нет."),
            );
            (*kind, values)
        })
        .collect()
}
