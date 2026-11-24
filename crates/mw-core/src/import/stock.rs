//! Перенос склада MyWork 2 в базу, импортированную до плана 8.1 (решение 17): склад тогда
//! сохранялся только следами (`legacy_rows`), а теперь из них получаются позиции склада и запчасти
//! заказов — так же, как при новом импорте.
//!
//! `mw-legacy` читает следы последнего импорта (`Core::legacy_stock_source`), разбирает их тем же
//! кодом, что и импорт, и отдаёт результат `Core::import_stock`, который пишет всё одной
//! транзакцией. Права — как у импорта (`settings_manage` и `users_manage`).

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use mw_db::entity::{dictionary_item, legacy_import, legacy_row, order, order_part, stock_item};
use mw_db::ids::{IdTable, set_next_id};
use mw_domain::dictionary::DictionaryKind;
use mw_domain::money::Money;
use mw_domain::search::normalize_search;
use mw_domain::validation::FieldError;
use sea_orm::sea_query::{Expr, ExprTrait};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, EntityTrait, NotSet, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::write::{bad, expect_count, insert_chunked, validate_stock, write_stock};
use super::{ExtraCostsChange, ImportedStock};
use crate::{Actor, Core, CoreError, NewAuditEntry, write_audit};

const STOCK: &str = "Stock";
const STOCK_MAP: &str = "StockMap";
const STOCK_PHOTO: &str = "Stock.Photo";
const SEQUENCE: &str = "sqlite_sequence";
const FILE: &str = "file";
const CATALOG: &str = "Catalog";
/// Отметка отчёта импорта «склад сохранён только следами»; перенос её снимает.
const STOCK_PRESERVED: &str = "stock_preserved";
/// Списки склада среди следов файлов.
const LISTS_PREFIX: &str = "settings/Stock/";
/// Номеров в одном `IN (…)`.
const ID_CHUNK: usize = 500;

/// Можно ли перенести склад из прошлого импорта.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct LegacyStockState {
    /// Последний импорт; пусто — импортов не было.
    #[schema(required = true)]
    pub import_id: Option<i64>,
    /// Строк `Stock` (позиций склада MyWork 2) в следах последнего импорта.
    pub stock_rows: u64,
    /// Строк `StockMap` (списаний со склада в заказы) в следах последнего импорта.
    pub stock_map_rows: u64,
    /// Перенос доступен: у последнего импорта есть строки `Stock`, в его отчёте ещё есть отметка
    /// `stock_preserved` (склад не переносили), а позиций склада и запчастей заказов в базе нет.
    pub available: bool,
}

/// Итог переноса склада из прошлого импорта.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct StockImportSummary {
    /// Импорт, из следов которого перенесён склад (его отчёт дополнен).
    pub import_id: i64,
    /// Создано позиций склада.
    pub stock_items: u64,
    /// Создано запчастей заказов.
    pub order_parts: u64,
    /// Заказов, из «Расходов» которых выделены запчасти.
    pub orders_changed: u64,
}

/// След прошлого импорта: откуда строка и её исходные значения.
#[derive(Debug, Clone, PartialEq)]
pub struct LegacySourceRow {
    pub source: String,
    pub source_key: String,
    pub raw: serde_json::Value,
}

/// Заказ, на который ссылается строка `StockMap`: «расходы» сейчас и след его строки `Catalog`
/// в прошлом импорте (пусто — заказ создан не этим импортом).
#[derive(Debug, Clone, PartialEq)]
pub struct LegacyOrderCosts {
    pub id: i64,
    pub extra_costs: Money,
    pub catalog: Option<serde_json::Value>,
}

/// Что нужно для переноса склада: следы последнего импорта (`Stock`, `StockMap`, счётчики
/// `sqlite_sequence`, файлы `settings/Stock/*`) по порядку, заказы из `StockMap`, справочники
/// (с архивными значениями, по порядку) и отчёт импорта.
#[derive(Debug, Clone, PartialEq)]
pub struct LegacyStockSource {
    pub import_id: i64,
    pub rows: Vec<LegacySourceRow>,
    pub orders: Vec<LegacyOrderCosts>,
    pub dictionary: Vec<(DictionaryKind, Vec<String>)>,
    pub report: serde_json::Value,
}

/// Разобранный склад прошлого импорта: позиции и запчасти, правки «расходов», добавки в
/// справочники, номера строк `Stock`, ставших позициями, ключи строк `StockMap`, ставших
/// запчастями (→ номер заказа), и новый отчёт импорта.
#[derive(Debug, Clone, PartialEq)]
pub struct StockImport {
    pub import_id: i64,
    pub stock: ImportedStock,
    pub cost_changes: Vec<ExtraCostsChange>,
    pub dictionary_additions: Vec<(DictionaryKind, Vec<String>)>,
    pub stock_targets: Vec<i64>,
    pub part_targets: Vec<(String, i64)>,
    pub report: serde_json::Value,
}

impl Core {
    /// Можно ли перенести склад из прошлого импорта (права `settings_manage` и `users_manage`).
    pub async fn legacy_stock_state(&self, actor: &Actor) -> Result<LegacyStockState, CoreError> {
        self.check_import_rights(actor)?;
        let Some(import_id) = latest_import(&self.db).await? else {
            return Ok(LegacyStockState {
                import_id: None,
                stock_rows: 0,
                stock_map_rows: 0,
                available: false,
            });
        };
        let stock_rows = trace_count(&self.db, import_id, STOCK).await?;
        let stock_map_rows = trace_count(&self.db, import_id, STOCK_MAP).await?;
        let available = stock_rows > 0
            && stock_is_empty(&self.db).await?
            && stock_preserved(&self.db, import_id).await?;
        Ok(LegacyStockState {
            import_id: Some(import_id),
            stock_rows,
            stock_map_rows,
            available,
        })
    }

    /// Следы последнего импорта для переноса склада (права `settings_manage` и `users_manage`).
    /// Импорта нет или в нём нет строк `Stock` — `stock` `not_available`. Пустоту склада и отметку
    /// `stock_preserved` здесь не проверяем: это делает `import_stock` внутри своей транзакции.
    /// `order_of` — номер заказа строки `StockMap` по её следу, как его читает разбор склада
    /// (`mw-legacy`): по нему выбираются заказы, «расходы» которых нужны.
    pub async fn legacy_stock_source(
        &self,
        actor: &Actor,
        order_of: fn(&Value) -> Option<i64>,
    ) -> Result<LegacyStockSource, CoreError> {
        self.check_import_rights(actor)?;
        let db = &self.db;
        let import_id = latest_import(db).await?.ok_or_else(not_available)?;
        if trace_count(db, import_id, STOCK).await? == 0 {
            return Err(not_available());
        }
        let traces: Vec<(String, String, String)> = legacy_row::Entity::find()
            .select_only()
            .column(legacy_row::Column::Source)
            .column(legacy_row::Column::SourceKey)
            .column(legacy_row::Column::Raw)
            .filter(legacy_row::Column::ImportId.eq(import_id))
            .filter(
                Condition::any()
                    .add(legacy_row::Column::Source.is_in([STOCK, STOCK_MAP, SEQUENCE]))
                    .add(
                        Condition::all()
                            .add(legacy_row::Column::Source.eq(FILE))
                            .add(legacy_row::Column::SourceKey.starts_with(LISTS_PREFIX)),
                    ),
            )
            .order_by_asc(legacy_row::Column::Id)
            .into_tuple()
            .all(db)
            .await?;
        let mut rows = Vec::with_capacity(traces.len());
        for (source, source_key, raw) in traces {
            // `LIKE` в SQLite не различает регистр: префикс проверяется ещё раз.
            if source == FILE && !source_key.starts_with(LISTS_PREFIX) {
                continue;
            }
            let raw = parse_raw(&source, &source_key, &raw)?;
            rows.push(LegacySourceRow {
                source,
                source_key,
                raw,
            });
        }
        let ids: BTreeSet<i64> = rows
            .iter()
            .filter(|r| r.source == STOCK_MAP)
            .filter_map(|r| order_of(&r.raw))
            .collect();
        let orders = order_costs(db, import_id, &ids).await?;
        let dictionary = dictionary_values(db).await?;
        let report = legacy_import::Entity::find_by_id(import_id)
            .one(db)
            .await?
            .ok_or_else(|| bad(format!("нет импорта №{import_id}")))?
            .report;
        let report = parse_raw("отчёт", &import_id.to_string(), &report)?;
        Ok(LegacyStockSource {
            import_id,
            rows,
            orders,
            dictionary,
            report,
        })
    }

    /// Пишет склад прошлого импорта одной транзакцией (права `settings_manage` и `users_manage`):
    /// позиции и запчасти, цели следов `Stock`, `Stock.Photo`, `StockMap`, фото позиций, правки
    /// «расходов» (только если они не менялись с чтения, иначе `Conflict`), добавки справочников,
    /// отчёт импорта и запись журнала. Импорт уже не последний или без склада — `stock`
    /// `not_available`; позиции склада или запчасти заказов уже есть — `stock` `not_empty`; склад
    /// этого импорта уже переносили (в отчёте нет `stock_preserved`) — `stock` `not_available`.
    pub async fn import_stock(
        &self,
        actor: &Actor,
        input: StockImport,
    ) -> Result<StockImportSummary, CoreError> {
        self.check_import_rights(actor)?;
        let referenced: BTreeSet<i64> = input
            .stock
            .parts
            .iter()
            .map(|p| p.order_id)
            .chain(input.cost_changes.iter().map(|c| c.order_id))
            .chain(input.part_targets.iter().map(|(_, id)| *id))
            .collect();
        // Существование заказов проверяется в транзакции; здесь — позиции, количества и цены.
        let item_ids = validate_stock(&input.stock, &referenced.iter().copied().collect())?;
        if input.stock_targets.iter().any(|id| !item_ids.contains(id)) {
            return Err(bad(
                "след позиции склада указывает на несуществующую позицию",
            ));
        }
        let mut changed = HashSet::new();
        for c in &input.cost_changes {
            if !changed.insert(c.order_id) || c.to.kopecks() < 0 || c.to > c.from {
                return Err(bad(format!(
                    "неверная правка расходов заказа №{}",
                    c.order_id
                )));
            }
        }
        if input
            .dictionary_additions
            .iter()
            .any(|(kind, _)| *kind == DictionaryKind::Master)
        {
            return Err(bad("мастера — не справочник"));
        }

        let tx = self.begin_write().await?;
        if latest_import(&tx).await? != Some(input.import_id)
            || trace_count(&tx, input.import_id, STOCK).await? == 0
        {
            return Err(not_available());
        }
        if !stock_is_empty(&tx).await? {
            return Err(not_empty());
        }
        // После проверки пустоты: повтор после переноса с позициями — по-прежнему `not_empty`, а
        // перенос, не создавший ничего, второй раз не проходит.
        if !stock_preserved(&tx, input.import_id).await? {
            return Err(not_available());
        }
        let orders = existing_orders(&tx, &referenced).await?;
        if let Some(id) = referenced.iter().find(|id| !orders.contains_key(id)) {
            return Err(bad(format!("заказа №{id} нет")));
        }

        let now = self.now();
        write_stock(&tx, &input.stock, now).await?;
        let parts: HashMap<&str, i64> = input
            .part_targets
            .iter()
            .map(|(key, id)| (key.as_str(), *id))
            .collect();
        let items: HashSet<i64> = input.stock_targets.iter().copied().collect();
        target_traces(&tx, input.import_id, &items, &parts).await?;
        mw_db::backfill::legacy_stock_photos(&tx).await?;

        for c in &input.cost_changes {
            let res = order::Entity::update_many()
                .col_expr(order::Column::ExtraCosts, Expr::value(c.to.kopecks()))
                .col_expr(order::Column::UpdatedAt, Expr::value(now))
                .col_expr(
                    order::Column::Version,
                    Expr::col(order::Column::Version).add(1),
                )
                .filter(order::Column::Id.eq(c.order_id))
                .filter(order::Column::ExtraCosts.eq(c.from.kopecks()))
                .exec(&tx)
                .await?;
            let (client_id, version) = orders[&c.order_id];
            if res.rows_affected == 0 {
                // «Расходы» изменили после чтения следов: повтор переноса прочитает их заново.
                return Err(CoreError::Conflict {
                    entity: "заказ",
                    id: c.order_id,
                    current_version: version,
                });
            }
            write_audit(
                &tx,
                now,
                actor,
                NewAuditEntry {
                    action: "order.costs_split",
                    order_id: Some(c.order_id),
                    client_id: Some(client_id),
                    stock_item_id: None,
                    summary: format!(
                        "Из расходов выделены запчасти со склада MyWork 2: было {}, стало {}",
                        rub_text(c.from),
                        rub_text(c.to)
                    ),
                    details: Some(json!({
                        "from": c.from.kopecks(),
                        "to": c.to.kopecks(),
                        "import_id": input.import_id,
                    })),
                },
            )
            .await?;
        }
        add_dictionary_values(&tx, &input.dictionary_additions).await?;

        let summary = StockImportSummary {
            import_id: input.import_id,
            stock_items: input.stock.items.len() as u64,
            order_parts: input.stock.parts.len() as u64,
            orders_changed: input.cost_changes.len() as u64,
        };
        let mut report = input.report;
        if let Some(Value::Object(map)) = report.get_mut("summary") {
            map.insert("stock_items".to_owned(), json!(summary.stock_items));
            map.insert("order_parts".to_owned(), json!(summary.order_parts));
        }
        legacy_import::ActiveModel {
            id: Set(input.import_id),
            report: Set(report.to_string()),
            ..Default::default()
        }
        .update(&tx)
        .await?;
        write_audit(
            &tx,
            now,
            actor,
            NewAuditEntry {
                action: "legacy.stock_imported",
                order_id: None,
                client_id: None,
                stock_item_id: None,
                summary: format!(
                    "Перенос склада из MyWork 2: позиций {}, запчастей {}",
                    summary.stock_items, summary.order_parts
                ),
                details: Some(json!({
                    "import_id": summary.import_id,
                    "stock_items": summary.stock_items,
                    "order_parts": summary.order_parts,
                    "orders_changed": summary.orders_changed,
                })),
            },
        )
        .await?;

        expect_count::<stock_item::Entity>(&tx, "позиции склада", summary.stock_items).await?;
        expect_count::<order_part::Entity>(&tx, "запчасти заказов", summary.order_parts).await?;
        // Последним: `setval` в PostgreSQL не откатывается вместе с транзакцией.
        let next = std::cmp::max(input.stock.next_stock_item_id, 1);
        set_next_id(&tx, IdTable::StockItems, next).await?;
        tx.commit().await?;
        Ok(summary)
    }
}

fn not_available() -> CoreError {
    CoreError::Validation(vec![FieldError::with_message(
        "stock",
        "not_available",
        "В последнем импорте нет склада MyWork 2 для переноса",
    )])
}

fn not_empty() -> CoreError {
    CoreError::Validation(vec![FieldError::with_message(
        "stock",
        "not_empty",
        "На складе или в заказах уже есть запчасти: перенос склада не нужен",
    )])
}

/// Сумма в журнале: «4 000,00 ₽» (простые пробелы, всегда копейки).
fn rub_text(m: Money) -> String {
    let abs = m.kopecks().unsigned_abs();
    let rubles = (abs / 100).to_string();
    let mut out = String::new();
    if m.kopecks() < 0 {
        out.push('-');
    }
    for (i, ch) in rubles.chars().enumerate() {
        if i > 0 && (rubles.len() - i).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(ch);
    }
    format!("{out},{:02} ₽", abs % 100)
}

fn parse_raw(source: &str, key: &str, raw: &str) -> Result<Value, CoreError> {
    serde_json::from_str(raw).map_err(|e| bad(format!("след {source}/{key}: {e}")))
}

/// Последний импорт (наибольший номер).
async fn latest_import(conn: &impl ConnectionTrait) -> Result<Option<i64>, CoreError> {
    Ok(legacy_import::Entity::find()
        .select_only()
        .column(legacy_import::Column::Id)
        .order_by_desc(legacy_import::Column::Id)
        .into_tuple::<i64>()
        .one(conn)
        .await?)
}

async fn trace_count(
    conn: &impl ConnectionTrait,
    import_id: i64,
    source: &str,
) -> Result<u64, CoreError> {
    Ok(legacy_row::Entity::find()
        .filter(legacy_row::Column::ImportId.eq(import_id))
        .filter(legacy_row::Column::Source.eq(source))
        .count(conn)
        .await?)
}

/// Позиций склада (и удалённых) и запчастей заказов нет.
async fn stock_is_empty(conn: &impl ConnectionTrait) -> Result<bool, CoreError> {
    Ok(stock_item::Entity::find().count(conn).await? == 0
        && order_part::Entity::find().count(conn).await? == 0)
}

/// В отчёте импорта ещё есть отметка `stock_preserved`: склад сохранён только следами и не
/// переносился. Отчёт, который не читается, отметки не содержит.
async fn stock_preserved(conn: &impl ConnectionTrait, import_id: i64) -> Result<bool, CoreError> {
    let report: Option<String> = legacy_import::Entity::find_by_id(import_id)
        .select_only()
        .column(legacy_import::Column::Report)
        .into_tuple()
        .one(conn)
        .await?;
    let Some(report) = report.and_then(|r| serde_json::from_str::<Value>(&r).ok()) else {
        return Ok(false);
    };
    Ok(report
        .get("warnings")
        .and_then(Value::as_array)
        .is_some_and(|list| {
            list.iter()
                .any(|w| w.get("code").and_then(Value::as_str) == Some(STOCK_PRESERVED))
        }))
}

/// «Расходы» заказов `ids`, которые есть в базе, и следы их строк `Catalog` в импорте `import_id`.
async fn order_costs(
    conn: &impl ConnectionTrait,
    import_id: i64,
    ids: &BTreeSet<i64>,
) -> Result<Vec<LegacyOrderCosts>, CoreError> {
    let ids: Vec<i64> = ids.iter().copied().collect();
    let mut costs: BTreeMap<i64, i64> = BTreeMap::new();
    let mut catalogs: HashMap<i64, Value> = HashMap::new();
    for chunk in ids.chunks(ID_CHUNK) {
        let found: Vec<(i64, i64)> = order::Entity::find()
            .select_only()
            .column(order::Column::Id)
            .column(order::Column::ExtraCosts)
            .filter(order::Column::Id.is_in(chunk.iter().copied()))
            .into_tuple()
            .all(conn)
            .await?;
        costs.extend(found);
        let traces: Vec<(Option<i64>, String, String)> = legacy_row::Entity::find()
            .select_only()
            .column(legacy_row::Column::TargetId)
            .column(legacy_row::Column::SourceKey)
            .column(legacy_row::Column::Raw)
            .filter(legacy_row::Column::ImportId.eq(import_id))
            .filter(legacy_row::Column::Source.eq(CATALOG))
            .filter(legacy_row::Column::TargetTable.eq("orders"))
            .filter(legacy_row::Column::TargetId.is_in(chunk.iter().copied()))
            .order_by_asc(legacy_row::Column::Id)
            .into_tuple()
            .all(conn)
            .await?;
        for (target, key, raw) in traces {
            if let Some(id) = target
                && !catalogs.contains_key(&id)
            {
                catalogs.insert(id, parse_raw(CATALOG, &key, &raw)?);
            }
        }
    }
    Ok(costs
        .into_iter()
        .map(|(id, extra_costs)| LegacyOrderCosts {
            id,
            extra_costs: Money::from_kopecks(extra_costs),
            catalog: catalogs.remove(&id),
        })
        .collect())
}

/// Значения справочников (и архивные) по видам, в порядке `sort_order`, затем номера.
async fn dictionary_values(
    conn: &impl ConnectionTrait,
) -> Result<Vec<(DictionaryKind, Vec<String>)>, CoreError> {
    let rows = dictionary_item::Entity::find()
        .order_by_asc(dictionary_item::Column::SortOrder)
        .order_by_asc(dictionary_item::Column::Id)
        .all(conn)
        .await?;
    let mut out: Vec<(DictionaryKind, Vec<String>)> = Vec::new();
    for row in rows {
        let Ok(kind) = row.kind.parse::<DictionaryKind>() else {
            continue;
        };
        match out.iter_mut().find(|(k, _)| *k == kind) {
            Some((_, values)) => values.push(row.value),
            None => out.push((kind, vec![row.value])),
        }
    }
    Ok(out)
}

/// Заказы `ids` из базы: номер → (клиент, версия).
async fn existing_orders(
    conn: &impl ConnectionTrait,
    ids: &BTreeSet<i64>,
) -> Result<HashMap<i64, (i64, i64)>, CoreError> {
    let ids: Vec<i64> = ids.iter().copied().collect();
    let mut out = HashMap::new();
    for chunk in ids.chunks(ID_CHUNK) {
        let found: Vec<(i64, i64, i64)> = order::Entity::find()
            .select_only()
            .column(order::Column::Id)
            .column(order::Column::ClientId)
            .column(order::Column::Version)
            .filter(order::Column::Id.is_in(chunk.iter().copied()))
            .into_tuple()
            .all(conn)
            .await?;
        out.extend(
            found
                .into_iter()
                .map(|(id, client, version)| (id, (client, version))),
        );
    }
    Ok(out)
}

/// Цели следов импорта, как у нового импорта: строка `Stock` и её фото (`Stock.Photo`, ключ
/// «номер/колонка») — позиция `items`, строка `StockMap` — заказ из `parts` (ключ → заказ).
async fn target_traces(
    conn: &impl ConnectionTrait,
    import_id: i64,
    items: &HashSet<i64>,
    parts: &HashMap<&str, i64>,
) -> Result<(), CoreError> {
    let traces: Vec<(i64, String, String)> = legacy_row::Entity::find()
        .select_only()
        .column(legacy_row::Column::Id)
        .column(legacy_row::Column::Source)
        .column(legacy_row::Column::SourceKey)
        .filter(legacy_row::Column::ImportId.eq(import_id))
        .filter(legacy_row::Column::Source.is_in([STOCK, STOCK_PHOTO, STOCK_MAP]))
        .order_by_asc(legacy_row::Column::Id)
        .into_tuple()
        .all(conn)
        .await?;
    let item = |id: &str| id.parse::<i64>().ok().filter(|id| items.contains(id));
    let mut targets: BTreeMap<(&'static str, i64), Vec<i64>> = BTreeMap::new();
    for (row_id, source, key) in traces {
        let target = match source.as_str() {
            STOCK => item(&key).map(|id| ("stock_items", id)),
            STOCK_PHOTO => key
                .rsplit_once('/')
                .and_then(|(id, _)| item(id))
                .map(|id| ("stock_items", id)),
            _ => parts.get(key.as_str()).map(|id| ("orders", *id)),
        };
        if let Some(target) = target {
            targets.entry(target).or_default().push(row_id);
        }
    }
    for ((table, target_id), rows) in targets {
        for chunk in rows.chunks(ID_CHUNK) {
            legacy_row::Entity::update_many()
                .col_expr(legacy_row::Column::TargetTable, Expr::value(table))
                .col_expr(legacy_row::Column::TargetId, Expr::value(target_id))
                .filter(legacy_row::Column::Id.is_in(chunk.iter().copied()))
                .exec(conn)
                .await?;
        }
    }
    Ok(())
}

/// Значения склада — в конец справочников (`sort_order` после последнего), кроме тех, что там уже
/// есть без учёта регистра.
async fn add_dictionary_values(
    conn: &impl ConnectionTrait,
    additions: &[(DictionaryKind, Vec<String>)],
) -> Result<(), CoreError> {
    for (kind, values) in additions {
        let rows = dictionary_item::Entity::find()
            .filter(dictionary_item::Column::Kind.eq(kind.as_str()))
            .all(conn)
            .await?;
        let mut known: HashSet<String> = rows.iter().map(|r| normalize_search(&r.value)).collect();
        let mut next = rows.iter().map(|r| r.sort_order).max().map_or(0, |m| m + 1);
        let mut added = Vec::new();
        for value in values {
            let key = normalize_search(value);
            if key.is_empty() || !known.insert(key) {
                continue;
            }
            added.push(dictionary_item::ActiveModel {
                id: NotSet,
                kind: Set(kind.as_str().to_owned()),
                value: Set(value.clone()),
                sort_order: Set(next),
                archived: Set(false),
            });
            next += 1;
        }
        insert_chunked::<dictionary_item::Entity>(conn, added).await?;
    }
    Ok(())
}
