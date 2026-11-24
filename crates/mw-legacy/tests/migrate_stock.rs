//! Перенос склада MyWork 2 в базу, импортированную до плана 8.1: кнопка «Перенести склад из
//! прошлого импорта» (решение 17).
#![allow(clippy::unwrap_used)]
mod common;

use std::path::Path;

use common::*;
use mw_core::{
    Actor, AttachmentKind, AuditFilter, Core, CoreError, LegacyStockState, NewStockItem,
    OrderPatch, SettingsPatch, StockFilter, StockImportSummary,
};
use mw_db::entity::{legacy_row, order, order_part};
use mw_domain::dictionary::{DEFAULT_DICTIONARY, DictionaryKind};
use mw_domain::money::Money;
use mw_domain::permission::{Permission, PermissionSet};
use mw_legacy::fixture::FixtureVersion::V32;
use mw_legacy::fixture::add_stock_oddities;
use mw_legacy::{ImportReport, LegacyError, Warning, import_folder, migrate_stock};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder};
use tempfile::TempDir;

type Res = Result<(), Box<dyn std::error::Error>>;

/// База, импортированная из фикстуры V32 так, как импортировала программа до плана 8.1: склада
/// нет, «расходы» заказов — затраты MyWork 2 как есть, следы склада без цели, в отчёте
/// `stock_preserved`; файлы разложены, как у `import_folder`. Вложения — в папке `data`.
async fn import_like_before_stock(core: &Core, data: &Path) -> Result<ImportReport, LegacyError> {
    let src = fixture_dir(V32).await;
    import_before_stock_from(core, src.path(), data).await
}

/// Как `import_like_before_stock`, из папки `src`.
async fn import_before_stock_from(
    core: &Core,
    src: &Path,
    data: &Path,
) -> Result<ImportReport, LegacyError> {
    mw_legacy::testing::import_before_stock(core, &admin(), &options(src, data, false)).await
}

/// Фикстура V32 с испорченным складом (`mw_legacy::fixture::add_stock_oddities`).
async fn odd_stock_fixture() -> TempDir {
    let src = fixture_dir(V32).await;
    add_stock_oddities(src.path()).await.unwrap();
    src
}

/// Строка отчёта о справочнике «Типы запчастей».
fn part_type_line(w: &Warning) -> bool {
    w.code.as_str() == "dictionary_from_orders" && w.key == "Типы запчастей"
}

/// Записи журнала с этим действием (по всей базе).
async fn audit_count(core: &Core, action: &str) -> usize {
    let filter = AuditFilter {
        action_prefix: Some(action.to_owned()),
        ..Default::default()
    };
    core.list_audit(&admin(), &filter, 100, 0)
        .await
        .unwrap()
        .items
        .len()
}

/// `Core` с папкой данных `data` (там лежат вложения импорта).
fn with_data(core: &Core, data: &TempDir) -> Core {
    core.clone().with_attachments_dir(data.path().to_path_buf())
}

fn rub(rubles: i64) -> Money {
    Money::from_rubles(rubles)
}

/// Отказ ядра с ошибкой поля: (поле, код).
fn field_error<T: std::fmt::Debug>(r: &Result<T, LegacyError>) -> Option<(String, String)> {
    match r {
        Err(LegacyError::Core(CoreError::Validation(fields))) if fields.len() == 1 => {
            Some((fields[0].field.clone(), fields[0].code.clone()))
        }
        _ => None,
    }
}

fn stock_error(code: &str) -> Option<(String, String)> {
    Some(("stock".to_owned(), code.to_owned()))
}

/// Предупреждения сохранённого отчёта импорта.
async fn stored_warnings(core: &Core, import_id: i64) -> Vec<Warning> {
    let record = core.legacy_import(&admin(), import_id).await.unwrap();
    serde_json::from_value(record.report["warnings"].clone()).unwrap()
}

fn sorted(mut warnings: Vec<Warning>) -> Vec<(String, String, String, String, String, String)> {
    let mut out: Vec<_> = warnings
        .drain(..)
        .map(|w| {
            (
                w.code.as_str().to_owned(),
                w.source,
                w.key,
                w.field,
                w.value,
                w.message,
            )
        })
        .collect();
    out.sort();
    out
}

/// Сводки журнала заказа с этим действием.
async fn audit_summaries(core: &Core, order_id: i64, action: &str) -> Vec<String> {
    let filter = AuditFilter {
        order_id: Some(order_id),
        action_prefix: Some(action.to_owned()),
        ..Default::default()
    };
    core.list_audit(&admin(), &filter, 100, 0)
        .await
        .unwrap()
        .items
        .into_iter()
        .map(|e| e.summary)
        .collect()
}

/// Следы склада: (источник, ключ, таблица цели, номер цели).
async fn stock_traces(core: &Core) -> Vec<(String, String, Option<String>, Option<i64>)> {
    legacy_row::Entity::find()
        .filter(legacy_row::Column::Source.is_in(["Stock", "StockMap", "Stock.Photo"]))
        .order_by_asc(legacy_row::Column::Source)
        .order_by_asc(legacy_row::Column::SourceKey)
        .all(core.db())
        .await
        .unwrap()
        .into_iter()
        .map(|r| (r.source, r.source_key, r.target_table, r.target_id))
        .collect()
}

async fn order_ids(core: &Core) -> Vec<i64> {
    order::Entity::find()
        .order_by_asc(order::Column::Id)
        .all(core.db())
        .await
        .unwrap()
        .into_iter()
        .map(|o| o.id)
        .collect()
}

async fn part_count(core: &Core) -> u64 {
    order_part::Entity::find().count(core.db()).await.unwrap()
}

#[tokio::test]
async fn state_offers_migration_only_when_stock_is_empty() -> Res {
    for (t, other) in cores().await.into_iter().zip(cores().await) {
        let label = t.label;
        // Импорта не было — переносить нечего.
        assert_eq!(
            t.core.legacy_stock_state(&admin()).await?,
            LegacyStockState {
                import_id: None,
                stock_rows: 0,
                stock_map_rows: 0,
                available: false,
                blocked_by_stock: false,
            },
            "{label}"
        );
        assert_eq!(
            field_error(&migrate_stock(&t.core, &admin()).await),
            stock_error("not_available"),
            "{label}"
        );

        let data = tempfile::tempdir()?;
        let core = with_data(&t.core, &data);
        let before = import_like_before_stock(&core, data.path()).await?;
        let id = before.summary.import_id;
        assert_eq!(
            core.legacy_stock_state(&admin()).await?,
            LegacyStockState {
                import_id: Some(id),
                stock_rows: 4,
                stock_map_rows: 6,
                available: true,
                blocked_by_stock: false,
            },
            "{label}"
        );
        migrate_stock(&core, &admin()).await?;
        let after = core.legacy_stock_state(&admin()).await?;
        assert_eq!(
            (
                after.import_id,
                after.stock_rows,
                after.available,
                after.blocked_by_stock
            ),
            (Some(id), 4, false, false),
            "{label}: перенесённый склад ничего не закрывает"
        );

        // Склад уже заводили руками: перенос не предлагается, и сам перенос отказывает; состояние
        // говорит, почему (страница «Импорт» объясняет, куда делась кнопка).
        let data = tempfile::tempdir()?;
        let core = with_data(&other.core, &data);
        import_like_before_stock(&core, data.path()).await?;
        let item = core
            .create_stock_item(
                &admin(),
                NewStockItem {
                    name: "Дисплей".into(),
                    ..Default::default()
                },
            )
            .await?;
        let state = core.legacy_stock_state(&admin()).await?;
        assert_eq!(
            (state.stock_rows, state.available, state.blocked_by_stock),
            (4, false, true),
            "{label}"
        );
        assert_eq!(
            field_error(&migrate_stock(&core, &admin()).await),
            stock_error("not_empty"),
            "{label}"
        );
        // Удалённая позиция тоже закрывает перенос: перенесённые позиции берут номера MyWork 2.
        core.delete_stock_item(&admin(), item.id, item.version)
            .await?;
        let state = core.legacy_stock_state(&admin()).await?;
        assert_eq!(
            (state.available, state.blocked_by_stock),
            (false, true),
            "{label}"
        );
    }
    Ok(())
}

/// Перенос даёт то же, что новый импорт той же папки (склад с испорченными ячейками и неизвестной
/// колонкой): позиции, фото, запчасти, «расходы», справочники, цели следов и предупреждения отчёта.
/// «Типы запчастей» старой базы — стандартные: значения склада дописываются в конец.
#[tokio::test]
async fn migration_matches_fresh_import() -> Res {
    for (old, new) in cores().await.into_iter().zip(cores().await) {
        let label = old.label;
        let (old_data, new_data) = (tempfile::tempdir()?, tempfile::tempdir()?);
        let migrated = with_data(&old.core, &old_data);
        let fresh = with_data(&new.core, &new_data);
        let old_src = odd_stock_fixture().await;
        let before = import_before_stock_from(&migrated, old_src.path(), old_data.path()).await?;
        let src = odd_stock_fixture().await;
        let imported = import_folder(
            &fresh,
            &admin(),
            &options(src.path(), new_data.path(), false),
        )
        .await?;
        // В старом отчёте нет строк о ячейках и колонках склада (их тогда не проверяли).
        let old_warnings = stored_warnings(&migrated, before.summary.import_id).await;
        assert!(
            !old_warnings
                .iter()
                .any(|w| w.source.starts_with("Stock") && w.code.as_str() == "text_invalid_utf8"),
            "{label}"
        );
        // До переноса: затраты как есть.
        assert_eq!(
            migrated.get_order(&admin(), 2).await?.extra_costs,
            rub(4000),
            "{label}"
        );

        let import_id = before.summary.import_id;
        let summary = migrate_stock(&migrated, &admin()).await?;
        assert_eq!(
            summary,
            StockImportSummary {
                import_id,
                stock_items: 4,
                order_parts: 6,
                orders_changed: 2,
            },
            "{label}"
        );

        let stock = |core: Core| async move {
            core.list_stock(&admin(), &StockFilter::default(), 50, 0)
                .await
                .unwrap()
                .items
        };
        let items = stock(migrated.clone()).await;
        assert_eq!(items.len(), 4, "{label}");
        assert_eq!(items, stock(fresh.clone()).await, "{label}");
        for id in 1..=4 {
            let photos = |core: Core| async move {
                core.list_stock_photos(&admin(), id)
                    .await
                    .unwrap()
                    .into_iter()
                    .map(|p| {
                        (
                            p.name,
                            p.kind,
                            p.content_type,
                            p.size,
                            p.legacy,
                            p.created_at,
                        )
                    })
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                photos(migrated.clone()).await,
                photos(fresh.clone()).await,
                "{label}: фото позиции {id}"
            );
        }
        assert_eq!(
            migrated.list_stock_photos(&admin(), 3).await?.len(),
            3,
            "{label}"
        );
        // Фото без точки в имени — всё равно фото; тип — по имени (`classify_file`): неизвестен.
        let no_dot: Vec<_> = migrated
            .list_stock_photos(&admin(), 4)
            .await?
            .into_iter()
            .map(|p| (p.name, p.kind, p.content_type, p.legacy))
            .collect();
        assert_eq!(
            no_dot,
            [(
                "p4jpeg".to_owned(),
                AttachmentKind::Photo,
                "application/octet-stream".to_owned(),
                true
            )],
            "{label}"
        );

        let ids = order_ids(&migrated).await;
        assert_eq!(ids, order_ids(&fresh).await);
        for id in ids {
            assert_eq!(
                migrated.list_order_parts(&admin(), id).await?,
                fresh.list_order_parts(&admin(), id).await?,
                "{label}: запчасти заказа {id}"
            );
            assert_eq!(
                migrated.get_order(&admin(), id).await?.extra_costs,
                fresh.get_order(&admin(), id).await?.extra_costs,
                "{label}: расходы заказа {id}"
            );
        }
        let order2 = migrated.get_order(&admin(), 2).await?;
        assert_eq!(order2.extra_costs, Money::from_kopecks(29_950));
        // Правка «расходов» — правка заказа: время правки — время переноса.
        assert_eq!(order2.updated_at, migrated.now(), "{label}");
        assert_ne!(
            migrated.get_order(&admin(), 4).await?.updated_at,
            migrated.now(),
            "{label}"
        );
        // Номер заказа списания 3 — «5» с NUL: запчасть у заказа 5, как у нового импорта.
        assert_eq!(
            migrated.list_order_parts(&admin(), 5).await?.parts.len(),
            1,
            "{label}"
        );
        // Пустой `ZIPId` списания 7: запчасть заказа 6 без позиции.
        let order6: Vec<_> = migrated
            .list_order_parts(&admin(), 6)
            .await?
            .parts
            .into_iter()
            .map(|p| (p.stock_item_id, p.name, p.price, p.quantity, p.legacy))
            .collect();
        assert_eq!(
            order6,
            [(
                None,
                "Позиция (нет на складе)".to_owned(),
                Money::ZERO,
                1,
                true
            )],
            "{label}"
        );

        for (kind, defaults) in DEFAULT_DICTIONARY {
            let values = |core: Core| async move {
                core.list_dictionary(*kind, true)
                    .await
                    .unwrap()
                    .into_iter()
                    .map(|i| (i.value, i.archived))
                    .collect::<Vec<_>>()
            };
            let expected = if *kind == DictionaryKind::PartType {
                // Стандартные значения остаются, тип позиции 1 («Дисплеи») — в конце; «АККУМУЛЯТОР»
                // и «ДИСПЛЕЙ» уже есть другим регистром.
                defaults
                    .iter()
                    .map(|v| ((*v).to_owned(), false))
                    .chain([("Дисплеи".to_owned(), false)])
                    .collect()
            } else {
                values(fresh.clone()).await
            };
            assert_eq!(
                values(migrated.clone()).await,
                expected,
                "{label}: справочник {kind:?}"
            );
        }

        let traces = stock_traces(&migrated).await;
        assert_eq!(traces, stock_traces(&fresh).await, "{label}");
        assert_eq!(
            legacy_target(&migrated, "StockMap", "3").await,
            Some(("orders".to_owned(), 5))
        );
        assert_eq!(
            legacy_target(&migrated, "Stock", "3").await,
            Some(("stock_items".to_owned(), 3))
        );
        assert_eq!(legacy_target(&migrated, "StockMap", "4").await, None);
        assert_eq!(
            legacy_target(&migrated, "StockMap", "7").await,
            Some(("orders".to_owned(), 6))
        );

        // Знак валюты — из настроек, перенесённых из `Config.ini` фикстуры (`valuta=руб.`), как у
        // записей «Списано со склада …» того же заказа.
        assert_eq!(
            audit_summaries(&migrated, 2, "order.costs_split").await,
            [
                "Из расходов выделены запчасти со склада MyWork 2: было 4 000,00 руб., стало 299,50 руб."
            ],
            "{label}"
        );
        assert_eq!(
            audit_summaries(&migrated, 5, "order.costs_split").await,
            [
                "Из расходов выделены запчасти со склада MyWork 2: было 500,00 руб., стало 200,00 руб."
            ],
            "{label}"
        );
        assert!(
            audit_summaries(&migrated, 4, "order.costs_split")
                .await
                .is_empty()
        );

        // Отчёт того импорта: без `stock_preserved`, с предупреждениями склада, как у нового импорта.
        let record = migrated.legacy_import(&admin(), import_id).await?;
        assert_eq!(
            (
                &record.report["summary"]["stock_items"],
                &record.report["summary"]["order_parts"]
            ),
            (&serde_json::json!(4), &serde_json::json!(6)),
            "{label}"
        );
        let warnings = stored_warnings(&migrated, import_id).await;
        assert!(
            !warnings
                .iter()
                .any(|w| w.code.as_str() == "stock_preserved"),
            "{label}"
        );
        assert!(
            warnings.iter().any(|w| w.code.as_str() == "stock_imported"
                && w.value == "позиций 4, запчастей 6"),
            "{label}"
        );
        // Испорченные ячейки и неизвестные колонки склада — в отчёте, по одному разу; списание с
        // пустым `ZIPId` — запчасть без позиции.
        for (code, source, key, field) in [
            ("text_invalid_utf8", "Stock", "2", "Primechanie"),
            ("text_invalid_utf8", "StockMap", "3", "clientId"),
            ("text_invalid_utf8", "StockMap", "1", "Mesto"),
            ("unknown_column", "Stock", "Garantiya", "Garantiya"),
            ("unknown_column", "StockMap", "Mesto", "Mesto"),
            ("stock_part_orphan", "StockMap", "7", "ZIPId"),
        ] {
            let found = warnings
                .iter()
                .filter(|w| {
                    (
                        w.code.as_str(),
                        w.source.as_str(),
                        w.key.as_str(),
                        w.field.as_str(),
                    ) == (code, source, key, field)
                })
                .count();
            assert_eq!(found, 1, "{label}: {code} {source} {key}");
        }
        // Строка о списании называет заказ: ключ строки `StockMap` человеку ничего не говорит.
        let orphan: Vec<_> = warnings
            .iter()
            .filter(|w| w.code.as_str() == "stock_part_orphan" && w.key == "7")
            .map(|w| w.message.as_str())
            .collect();
        assert_eq!(
            orphan,
            ["Заказ № 6: номер позиции склада не указан — запчасть перенесена без позиции."],
            "{label}"
        );
        // «Типы запчастей»: строки отчёта у старой базы не было — перенос её заводит.
        let line: Vec<_> = warnings.iter().filter(|w| part_type_line(w)).collect();
        assert_eq!(line.len(), 1, "{label}");
        assert_eq!(line[0].value, "в программе — 11, со склада — 1", "{label}");
        let without_part_types =
            |list: Vec<Warning>| sorted(list.into_iter().filter(|w| !part_type_line(w)).collect());
        assert_eq!(
            without_part_types(warnings),
            without_part_types(imported.warnings.clone()),
            "{label}"
        );
        let view = mw_legacy::report_view(&record, None);
        assert_eq!(view.stock_items, 4, "{label}");

        // Следующая позиция — после счётчика `Stock` (20), а не после наибольшего номера (4).
        let next = |core: Core| async move {
            core.create_stock_item(
                &admin(),
                NewStockItem {
                    name: "Шлейф".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap()
            .id
        };
        assert_eq!(next(migrated.clone()).await, 21, "{label}");
        assert_eq!(next(fresh.clone()).await, 21, "{label}");
    }
    Ok(())
}

/// «Расходы», изменённые после импорта, не трогаются: запчасти переносятся, в отчёте — пояснение.
#[tokio::test]
async fn costs_changed_after_import_are_kept() -> Res {
    for t in cores().await {
        let label = t.label;
        let data = tempfile::tempdir()?;
        let core = with_data(&t.core, &data);
        let before = import_like_before_stock(&core, data.path()).await?;
        let order = core.get_order(&admin(), 2).await?;
        core.update_order(
            &admin(),
            2,
            order.version,
            OrderPatch {
                extra_costs: Some(rub(4100)),
                ..Default::default()
            },
        )
        .await?;

        let summary = migrate_stock(&core, &admin()).await?;
        assert_eq!(summary.orders_changed, 1, "{label}");
        assert_eq!(core.get_order(&admin(), 2).await?.extra_costs, rub(4100));
        assert_eq!(core.get_order(&admin(), 5).await?.extra_costs, rub(200));
        assert_eq!(core.list_order_parts(&admin(), 2).await?.parts.len(), 2);
        assert!(
            audit_summaries(&core, 2, "order.costs_split")
                .await
                .is_empty()
        );

        let kept: Vec<_> = stored_warnings(&core, before.summary.import_id)
            .await
            .into_iter()
            .filter(|w| w.code.as_str() == "extra_costs_kept")
            .map(|w| (w.key, w.value, w.message.contains("менялись после импорта")))
            .collect();
        assert_eq!(
            kept,
            [
                (
                    "2".to_owned(),
                    "затраты 4 100,00, запчасти 3 700,50".to_owned(),
                    true
                ),
                (
                    "4".to_owned(),
                    "затраты 1 000,00, запчасти 5 000,00".to_owned(),
                    false
                ),
            ],
            "{label}"
        );
    }
    Ok(())
}

/// Запись журнала о «Расходах» пишет сумму со знаком валюты из настроек, как записи о запчастях:
/// у мастерской в тенге — тенге, а не рубли рядом с «Списано со склада … ₸». Вид суммы прежний:
/// простые пробелы и всегда копейки.
#[tokio::test]
async fn costs_split_text_uses_the_currency_setting() -> Res {
    for (symbol, shown) in [(" ₸ ", "₸"), ("₽", "₽")] {
        for t in cores().await {
            let label = t.label;
            let data = tempfile::tempdir()?;
            let core = with_data(&t.core, &data);
            import_like_before_stock(&core, data.path()).await?;
            core.update_settings(
                &admin(),
                SettingsPatch {
                    currency_symbol: Some(symbol.into()),
                    ..Default::default()
                },
            )
            .await?;
            migrate_stock(&core, &admin()).await?;
            assert_eq!(
                audit_summaries(&core, 2, "order.costs_split").await,
                [format!(
                    "Из расходов выделены запчасти со склада MyWork 2: было 4 000,00 {shown}, стало 299,50 {shown}"
                )],
                "{label}"
            );
        }
    }
    Ok(())
}

/// Повтор переноса (второй щелчок, вторая вкладка) отказывает: запчасти и вычеты из «расходов» не
/// задваиваются (Review Focus 5).
#[tokio::test]
async fn second_run_is_refused() -> Res {
    for (t, other) in cores().await.into_iter().zip(cores().await) {
        let label = t.label;
        let data = tempfile::tempdir()?;
        let core = with_data(&t.core, &data);
        import_like_before_stock(&core, data.path()).await?;
        migrate_stock(&core, &admin()).await?;
        assert_eq!(
            field_error(&migrate_stock(&core, &admin()).await),
            stock_error("not_empty"),
            "{label}"
        );
        assert_eq!(part_count(&core).await, 5, "{label}");

        // Два одновременно: проходит один.
        let data = tempfile::tempdir()?;
        let core = with_data(&other.core, &data);
        import_like_before_stock(&core, data.path()).await?;
        let actor = admin();
        let (a, b) = tokio::join!(migrate_stock(&core, &actor), migrate_stock(&core, &actor));
        let refused = [&a, &b]
            .iter()
            .filter(|r| field_error(r) == stock_error("not_empty"))
            .count();
        assert!(
            a.is_ok() != b.is_ok() && refused == 1,
            "{label}: {a:?} / {b:?}"
        );
        assert_eq!(part_count(&core).await, 5, "{label}");
        assert_eq!(
            core.list_stock(&admin(), &StockFilter::default(), 50, 0)
                .await?
                .items
                .len(),
            4,
            "{label}"
        );
        assert_eq!(
            core.get_order(&admin(), 2).await?.extra_costs,
            Money::from_kopecks(29_950),
            "{label}"
        );
        assert_eq!(
            audit_summaries(&core, 2, "order.costs_split").await.len(),
            1,
            "{label}"
        );
    }
    Ok(())
}

/// Перенос, который не создал ни позиции, ни запчасти, второй раз не предлагается и не проходит:
/// предложение держится на отметке `stock_preserved` в отчёте импорта, а перенос её снимает.
#[tokio::test]
async fn migration_that_moved_nothing_is_not_repeated() -> Res {
    for t in cores().await {
        let label = t.label;
        let data = tempfile::tempdir()?;
        let core = with_data(&t.core, &data);
        // Номера позиций не больше нуля, заказов списаний нет: переносить нечего.
        let src = fixture_dir(V32).await;
        run_sql(
            src.path(),
            &[
                "UPDATE Stock SET id = -id",
                "UPDATE StockMap SET clientId = '404'",
            ],
        )
        .await;
        let before = import_before_stock_from(&core, src.path(), data.path()).await?;
        let id = before.summary.import_id;
        assert!(
            core.legacy_stock_state(&admin()).await?.available,
            "{label}"
        );

        let summary = migrate_stock(&core, &admin()).await?;
        assert_eq!(
            summary,
            StockImportSummary {
                import_id: id,
                stock_items: 0,
                order_parts: 0,
                orders_changed: 0,
            },
            "{label}"
        );
        let state = core.legacy_stock_state(&admin()).await?;
        assert_eq!(
            (state.stock_rows, state.available, state.blocked_by_stock),
            (4, false, false),
            "{label}"
        );
        assert_eq!(
            field_error(&migrate_stock(&core, &admin()).await),
            stock_error("not_available"),
            "{label}"
        );
        assert_eq!(
            audit_count(&core, "legacy.stock_imported").await,
            1,
            "{label}"
        );
        let imported = stored_warnings(&core, id)
            .await
            .into_iter()
            .filter(|w| w.code.as_str() == "stock_imported")
            .count();
        assert_eq!(imported, 1, "{label}");
    }
    Ok(())
}

#[tokio::test]
async fn rights() -> Res {
    for t in cores().await {
        let label = t.label;
        let data = tempfile::tempdir()?;
        let core = with_data(&t.core, &data);
        import_like_before_stock(&core, data.path()).await?;
        let mut set = PermissionSet::empty();
        for p in PermissionSet::all()
            .iter()
            .filter(|p| *p != Permission::UsersManage)
        {
            set.insert(p);
        }
        let weak = Actor::for_tests(None, "Тест", set);
        assert!(
            matches!(
                migrate_stock(&core, &weak).await,
                Err(LegacyError::Core(CoreError::Forbidden(
                    Permission::UsersManage
                )))
            ),
            "{label}"
        );
        // Состояние переноса читают и те, кто видит склад: «Склад» предупреждает их до первой позиции.
        assert!(core.legacy_stock_state(&weak).await?.available, "{label}");
        let viewer = Actor::for_tests(None, "Тест", PermissionSet::from_codes(["stock_view"]));
        assert!(core.legacy_stock_state(&viewer).await?.available, "{label}");
        assert!(
            matches!(
                migrate_stock(&core, &viewer).await,
                Err(LegacyError::Core(CoreError::Forbidden(
                    Permission::SettingsManage
                )))
            ),
            "{label}"
        );
        let mut blind = PermissionSet::empty();
        blind.insert(Permission::SettingsManage);
        let blind = Actor::for_tests(None, "Тест", blind);
        assert!(
            matches!(
                core.legacy_stock_state(&blind).await,
                Err(CoreError::Forbidden(Permission::UsersManage))
            ),
            "{label}"
        );
        assert_eq!(part_count(&core).await, 0, "{label}");
        assert!(
            core.legacy_stock_state(&admin()).await?.available,
            "{label}"
        );
    }
    Ok(())
}
