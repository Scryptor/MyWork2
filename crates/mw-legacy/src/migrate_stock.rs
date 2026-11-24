//! Перенос склада MyWork 2 в базу, импортированную до плана 8.1 (решение 17): тогда склад
//! сохранялся только следами (`legacy_rows`). Следы последнего импорта снова становятся таблицами
//! `Stock` и `StockMap` и разбираются тем же кодом, что и при новом импорте
//! (`convert::stock::convert_stock`); пишет всё одной транзакцией `Core::import_stock`.

use std::collections::BTreeMap;

use mw_core::{Actor, Core, CoreError, LegacyStockSource, StockImport, StockImportSummary};
use serde_json::Value;

use crate::convert::dictionaries::append_stock_values;
use crate::convert::stock::{OrderCosts, StockSource, convert_stock, list_name};
use crate::error::LegacyError;
use crate::parse::money::{MoneyValue, parse_money};
use crate::report::{Warning, WarningCode, Warnings};
use crate::source::LegacyTable;

/// Переносит склад из следов последнего импорта так, как его перенёс бы новый импорт той же папки
/// (права `settings_manage` и `users_manage`). Импорта со складом нет — `stock` `not_available`;
/// склад или запчасти заказов уже есть — `stock` `not_empty`.
pub async fn migrate_stock(core: &Core, actor: &Actor) -> Result<StockImportSummary, LegacyError> {
    let source = core.legacy_stock_source(actor).await?;
    let input = stock_import(source)?;
    Ok(core.import_stock(actor, input).await?)
}

/// Склад из следов: позиции, запчасти, правки «расходов», добавки справочников и отчёт импорта
/// без `stock_preserved`, с предупреждениями склада.
fn stock_import(source: LegacyStockSource) -> Result<StockImport, CoreError> {
    let table = |name: &str| {
        let rows: Vec<Value> = source
            .rows
            .iter()
            .filter(|r| r.source == name)
            .map(|r| r.raw.clone())
            .collect();
        LegacyTable::from_json_rows(name, &rows)
    };
    let stock = table("Stock");
    let stock_map = table("StockMap");
    let lists: BTreeMap<String, String> = source
        .rows
        .iter()
        .filter(|r| r.source == "file")
        .filter_map(|r| {
            let name = list_name(&r.source_key)?;
            Some((name.to_owned(), r.raw.get("text")?.as_str()?.to_owned()))
        })
        .collect();
    // Как `sequence_of` нового импорта: первый счётчик таблицы `Stock` без учёта регистра.
    let sequence = source
        .rows
        .iter()
        .find(|r| r.source == "sqlite_sequence" && r.source_key.eq_ignore_ascii_case("Stock"))
        .and_then(|r| r.raw.get("seq")?.as_i64())
        .unwrap_or(0);
    // «Расходы» меняются, только если сейчас равны тому, что записал импорт из `Zatrati`.
    let orders: BTreeMap<i64, OrderCosts> = source
        .orders
        .iter()
        .filter_map(|o| {
            let catalog =
                LegacyTable::from_json_rows("Catalog", std::slice::from_ref(o.catalog.as_ref()?));
            let zatrati = catalog
                .rows
                .first()
                .map(|row| catalog.text(row, "Zatrati"))
                .unwrap_or_default();
            let imported = match parse_money(&zatrati) {
                MoneyValue::Parsed(m) => m,
                MoneyValue::Empty | MoneyValue::Invalid => mw_domain::money::Money::ZERO,
            };
            let costs = OrderCosts {
                current: o.extra_costs,
                imported,
            };
            Some((o.id, costs))
        })
        .collect();

    let stored: Vec<Warning> = match source.report.get("warnings") {
        Some(list) => serde_json::from_value(list.clone())
            .map_err(|e| CoreError::Internal(format!("отчёт импорта не разобран: {e}")))?,
        None => Vec::new(),
    };
    let mut w = Warnings::from(
        stored
            .into_iter()
            .filter(|x| x.code != WarningCode::StockPreserved)
            .collect::<Vec<_>>(),
    );
    let src = StockSource {
        stock: Some(&stock),
        stock_map: Some(&stock_map),
        lists: &lists,
        orders: &orders,
        sequence,
    };
    let converted = convert_stock(&src, &source.dictionary, &mut w);
    // Справочники здесь не пишутся: нужны только строки отчёта о них («со склада — N»).
    let mut dictionary = source.dictionary.clone();
    append_stock_values(&mut dictionary, &converted.dictionary_additions, &mut w);

    let mut report = source.report;
    let warnings = serde_json::to_value(w.into_list())
        .map_err(|e| CoreError::Internal(format!("отчёт импорта: {e}")))?;
    if let Value::Object(map) = &mut report {
        map.insert("warnings".to_owned(), warnings);
    }
    Ok(StockImport {
        import_id: source.import_id,
        stock: converted.stock,
        cost_changes: converted.cost_changes,
        dictionary_additions: converted.dictionary_additions,
        stock_targets: converted.stock_targets,
        part_targets: converted.part_targets,
        report,
    })
}
