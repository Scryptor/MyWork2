//! Замена значения справочника в заказах и клиентах.
//!
//! Каждая изменённая запись проходит обычную правку (`update_order_in` / `update_client_in`):
//! версия, `updated_at`, `search_text`, ключи, срок гарантии и журнал — те же, что при правке руками,
//! с пометкой «из справочника». Срок гарантии заказа при этом не меняется: замена, которая дала бы
//! другой срок, отклоняется целиком.

use std::collections::HashSet;

use mw_domain::dictionary::DictionaryKind;
use mw_domain::search::normalize_search;
use mw_domain::validation::FieldError;
use mw_domain::warranty::parse_warranty_term;
use sea_orm::{ConnectionTrait, DatabaseTransaction, Statement};

use crate::usage::usage_target;
use crate::{Actor, ClientPatch, Core, CoreError, EditVia, OrderPatch};

/// Новый текст поля, если в нём есть значение с ключом из `from`; иначе `None`.
///
/// Поле целиком сравнивается по ключу. В поле-списке заменяются только целые элементы, повторы
/// нового значения схлопываются (первое вхождение остаётся на месте), пустые элементы и пробелы
/// по краям уходят, разделитель — «, »; остальные элементы и их порядок не меняются.
pub(crate) fn replace_value(
    text: &str,
    list: bool,
    from: &HashSet<String>,
    to: &str,
) -> Option<String> {
    if !list {
        return from
            .contains(&normalize_search(text))
            .then(|| to.to_owned());
    }
    let to_key = normalize_search(to);
    let mut matched = false;
    let mut seen_to = false;
    let mut out: Vec<&str> = Vec::new();
    for part in text.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let key = normalize_search(part);
        let replaced = from.contains(&key);
        matched |= replaced;
        if replaced || key == to_key {
            if seen_to {
                continue;
            }
            seen_to = true;
        }
        out.push(if replaced { to } else { part });
    }
    matched.then(|| out.join(", "))
}

/// Правка одной записи: поле, в котором хранится значение справочника этого вида.
enum FieldPatch {
    Order(Box<OrderPatch>),
    Client(ClientPatch),
}

/// То же поле, что читает `usage_target(kind)`. Мастер заказа — пользователь: справочник «Мастера»
/// заказы не меняет (`None`).
fn field_patch(kind: DictionaryKind, value: String) -> Option<FieldPatch> {
    let v = Some(value);
    let order = |p: OrderPatch| FieldPatch::Order(Box::new(p));
    Some(match kind {
        DictionaryKind::DeviceType => order(OrderPatch {
            device_type: v,
            ..Default::default()
        }),
        DictionaryKind::Brand => order(OrderPatch {
            brand: v,
            ..Default::default()
        }),
        DictionaryKind::Color => order(OrderPatch {
            color: v,
            ..Default::default()
        }),
        DictionaryKind::Completeness => order(OrderPatch {
            completeness: v,
            ..Default::default()
        }),
        DictionaryKind::Condition => order(OrderPatch {
            device_condition: v,
            ..Default::default()
        }),
        DictionaryKind::Malfunction => order(OrderPatch {
            malfunction: v,
            ..Default::default()
        }),
        DictionaryKind::WorkDone => order(OrderPatch {
            work_done: v,
            ..Default::default()
        }),
        DictionaryKind::Master => return None,
        DictionaryKind::Warranty => order(OrderPatch {
            warranty: v,
            ..Default::default()
        }),
        DictionaryKind::ClientSource => FieldPatch::Client(ClientPatch {
            source: v,
            ..Default::default()
        }),
    })
}

/// «заказов» или «клиентов» — что меняет справочник этого вида.
pub(crate) fn records_word(kind: DictionaryKind) -> &'static str {
    match kind {
        DictionaryKind::ClientSource => "клиентов",
        _ => "заказов",
    }
}

impl Core {
    /// Во всех неудалённых записях вида `kind` значения (элементы списка) с ключом из `from`
    /// заменяет на `to`; возвращает число изменённых записей. Неизменившиеся записи не трогает.
    /// Гарантия: если у какого-нибудь заказа срок, выведенный из нового текста, отличается от
    /// нынешнего, — ошибка `changes_term` у поля `term_field` (транзакцию откатывает вызывающий).
    pub(crate) async fn rewrite_dictionary_values(
        &self,
        tx: &DatabaseTransaction,
        actor: &Actor,
        kind: DictionaryKind,
        from: &HashSet<String>,
        to: &str,
        term_field: &'static str,
    ) -> Result<u64, CoreError> {
        crate::dictionaries::refuse_closed(kind)?;
        let target = usage_target(kind);
        // Имена берутся только из статической таблицы `usage_target`.
        let sql = format!(
            r#"SELECT "id", "version", "{}" FROM "{}" WHERE "deleted_at" IS NULL ORDER BY "id""#,
            target.column, target.table
        );
        let rows = tx
            .query_all_raw(Statement::from_string(tx.get_database_backend(), sql))
            .await?;
        let mut changed = 0;
        for row in rows {
            let id = row.try_get_by_index::<i64>(0)?;
            let version = row.try_get_by_index::<i64>(1)?;
            let text = row
                .try_get_by_index::<Option<String>>(2)?
                .unwrap_or_default();
            let Some(value) = replace_value(&text, target.list, from, to) else {
                continue;
            };
            if value == text {
                continue;
            }
            if kind == DictionaryKind::Warranty
                && parse_warranty_term(&text) != parse_warranty_term(&value)
            {
                return Err(CoreError::Validation(vec![FieldError::new(
                    term_field,
                    "changes_term",
                )]));
            }
            let via = Some(EditVia::Dictionary);
            let Some(patch) = field_patch(kind, value) else {
                return Err(crate::dictionaries::closed_error());
            };
            let edited = match patch {
                FieldPatch::Order(patch) => {
                    self.update_order_in(tx, actor, id, version, *patch, via)
                        .await?
                        .1
                }
                FieldPatch::Client(patch) => {
                    self.update_client_in(tx, actor, id, version, patch, via)
                        .await?
                        .1
                }
            };
            if edited {
                changed += 1;
            }
        }
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(values: &[&str]) -> HashSet<String> {
        values.iter().map(|v| normalize_search(v)).collect()
    }

    fn list(text: &str, from: &[&str], to: &str) -> Option<String> {
        replace_value(text, true, &keys(from), to)
    }

    #[test]
    fn whole_value_matches_by_key() {
        let from = keys(&["samsung"]);
        assert_eq!(
            replace_value(" SAMSUNG ", false, &from, "Samsung Electronics").as_deref(),
            Some("Samsung Electronics")
        );
        assert_eq!(replace_value("Samsung S", false, &from, "X"), None);
        assert_eq!(replace_value("", false, &from, "X"), None);
    }

    #[test]
    fn list_replaces_whole_elements_and_collapses_repeats() {
        assert_eq!(
            list("чехол, Зарядка, Чехол", &["чехол"], "Чехол").as_deref(),
            Some("Чехол, Зарядка")
        );
        assert_eq!(
            list("Кабель,чехол ,, Коробка", &["чехол"], "Чехол").as_deref(),
            Some("Кабель, Чехол, Коробка")
        );
        assert_eq!(
            list("Чехол силиконовый, Зарядка", &["чехол"], "Чехол"),
            None
        );
        // Уже стоящее новое значение — тоже повтор: остаётся первое вхождение.
        assert_eq!(
            list("Зарядка, чехлы, Чехол", &["чехлы"], "Чехол").as_deref(),
            Some("Зарядка, Чехол")
        );
        assert_eq!(
            list("чехол, чехлы", &["чехлы"], "Чехол").as_deref(),
            Some("чехол")
        );
        // Посторонние повторы не трогаются.
        assert_eq!(
            list("Зарядка, зарядка, чехлы", &["чехлы"], "Чехол").as_deref(),
            Some("Зарядка, зарядка, Чехол")
        );
        // Совпало, но текст тот же — вызывающий сравнит и запись не тронет.
        assert_eq!(
            list("Чехол, Зарядка", &["чехол"], "Чехол").as_deref(),
            Some("Чехол, Зарядка")
        );
    }

    #[test]
    fn records_word_names_clients_for_source() {
        assert_eq!(records_word(DictionaryKind::ClientSource), "клиентов");
        assert_eq!(records_word(DictionaryKind::Brand), "заказов");
    }
}
