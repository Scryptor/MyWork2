//! Справочники MyWork 3 при переносе: значения, которые писали в заказах (и у клиентов — «Откуда
//! узнали»), затем значения списков `settings/*.txt`, которых в заказах нет (файла нет или он не
//! менялся — стандартные значения MyWork 3; `super::catalogs`).
//!
//! Поля — те же, что у счёта использования (`usage_target` в `mw-core/src/usage.rs`). Поля-списки
//! заказа к этому моменту уже в виде MyWork 3 «пункт, пункт» (`crate::parse::lists`) и делятся так
//! же, как там (`mw_domain::dictionary::list_items`). Удалённые заказы не считаются, как и там.
//! - Короткие поля (тип устройства, бренд, цвет, гарантия, «Откуда узнали») — каждое значение;
//!   поля-списки — пункт хотя бы из двух заказов (одноразовые фразы в справочник не идут).
//! - Значения сводятся без учёта регистра (`normalize_search`), у списков — ещё и без конечных
//!   «.», «;», «/», «,» (и в строках файла); гарантии с одним сроком (`parse_warranty_term`:
//!   «90» и «90 дней») — одно значение. Текст без букв («-», «?», «0») — не значение, кроме гарантии
//!   со сроком («90» — 90 дней).
//! - Написание — из списка, если там есть такое значение, иначе самое частое в заказах, при
//!   равенстве — первое (`mw_domain::spelling::Spellings::most_frequent`; заглавные не уступают:
//!   «LG»); у гарантии — с единицей срока, если такое есть («30 дней», а не «30»).
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
use crate::parse::lists::clean_item;
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
    fn add_record(&mut self, kind: DictionaryKind, items: impl IntoIterator<Item = String>) {
        let mut seen = HashSet::new();
        for item in items {
            let key = value_key(kind, &item);
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

/// Ключ сведения значения: без учёта регистра; у гарантии со сроком — сам срок.
fn value_key(kind: DictionaryKind, text: &str) -> String {
    match parse_warranty_term(text).filter(|_| kind == DictionaryKind::Warranty) {
        Some(term) => format!("\u{0}{}:{}", term.unit.as_str(), term.value),
        None => normalize_search(text),
    }
}

/// Поле-список ли это (как `usage_target`).
fn is_list(kind: DictionaryKind) -> bool {
    matches!(
        kind,
        DictionaryKind::Completeness
            | DictionaryKind::Condition
            | DictionaryKind::Malfunction
            | DictionaryKind::WorkDone
    )
}

/// Значение ли это: есть буква; у гарантии — ещё и срок из одних цифр («90»).
fn is_value(kind: DictionaryKind, text: &str) -> bool {
    has_letter(text)
        || (kind == DictionaryKind::Warranty
            && parse_warranty_term(text).is_some_and(|term| term.value > 0))
}

/// Тексты вида справочника по записям MyWork 3 (как `usage_target`); `None` — мастера (они
/// пользователи).
fn texts<'a>(
    kind: DictionaryKind,
    orders: &[&'a ImportedOrder],
    clients: &[&'a ImportedClient],
) -> Option<Vec<&'a str>> {
    let of = |field: fn(&'a ImportedOrder) -> &'a str| orders.iter().map(|o| field(o)).collect();
    Some(match kind {
        DictionaryKind::DeviceType => of(|o| &o.device_type),
        DictionaryKind::Brand => of(|o| &o.brand),
        DictionaryKind::Color => of(|o| &o.color),
        DictionaryKind::Warranty => of(|o| &o.warranty),
        DictionaryKind::Completeness => of(|o| &o.completeness),
        DictionaryKind::Condition => of(|o| &o.device_condition),
        DictionaryKind::Malfunction => of(|o| &o.malfunction),
        DictionaryKind::WorkDone => of(|o| &o.work_done),
        DictionaryKind::ClientSource => clients.iter().map(|c| c.source.as_str()).collect(),
        DictionaryKind::Master => return None,
    })
}

/// Справочник одного вида и сколько в нём значений из заказов и из списка; у списков — ещё сколько
/// пунктов нашлось только в одном заказе.
struct Merged {
    values: Vec<String>,
    from_orders: usize,
    from_list: usize,
    once: usize,
}

fn merge(kind: DictionaryKind, texts: &[&str], listed: &ListedValues) -> Merged {
    let list = is_list(kind);
    let mut tally = Tally::default();
    for text in texts {
        let items: Vec<String> = if list {
            list_items(text).map(clean_item).collect()
        } else {
            vec![tidy(text)]
        };
        tally.add_record(kind, items.into_iter().filter(|item| is_value(kind, item)));
    }
    let least = if list { 2 } else { 1 };
    let mut used: Vec<&Counted> = tally.values.iter().filter(|c| c.records >= least).collect();
    // Сортировка устойчивая: при равном числе — по первому появлению.
    used.sort_by_key(|c| Reverse(c.records));

    // Строки списка: у списков — без конечных знаков препинания, как пункты заказов.
    let listed_values: Vec<String> = listed
        .values
        .iter()
        .map(|v| if list { clean_item(v) } else { v.clone() })
        .filter(|v| !v.is_empty())
        .collect();
    let mut by_key: HashMap<String, &str> = HashMap::new();
    for value in &listed_values {
        by_key.entry(value_key(kind, value)).or_insert(value);
    }
    let mut taken = HashSet::new();
    let mut values = Vec::new();
    for c in used {
        let spelling = by_key
            .get(&c.key)
            .copied()
            .or_else(|| {
                if kind == DictionaryKind::Warranty {
                    c.spellings.most_frequent_among(has_letter)
                } else {
                    c.spellings.most_frequent()
                }
            })
            .unwrap_or_default();
        taken.insert(c.key.clone());
        values.push(spelling.to_owned());
    }
    let from_orders = values.len();
    for value in listed_values {
        if taken.insert(value_key(kind, &value)) {
            values.push(value);
        }
    }
    Merged {
        from_list: values.len() - from_orders,
        values,
        from_orders,
        once: tally.values.iter().filter(|c| c.records < least).count(),
    }
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
            let Some(texts) = texts(*kind, &live, &clients) else {
                return (*kind, Vec::new());
            };
            let list = is_list(*kind);
            let m = merge(*kind, &texts, list_values);
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
            let mut counts = format!("{records} — {}, {rest} — {}", m.from_orders, m.from_list);
            if list {
                counts.push_str(&format!(", пунктов из одного заказа — {}", m.once));
            }
            w.push(
                WarningCode::DictionaryFromOrders,
                "dictionary",
                kind.label_ru(),
                "",
                &counts,
                &format!("{first}, затем {rest_text}, которых среди них нет."),
            );
            (*kind, m.values)
        })
        .collect()
}
