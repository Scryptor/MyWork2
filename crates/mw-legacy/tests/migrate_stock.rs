//! Перенос склада MyWork 2 в базу, импортированную до плана 8.1: кнопка «Перенести склад из
//! прошлого импорта» (решение 17).
#![allow(clippy::unwrap_used)]
mod common;

use std::path::Path;

use common::*;
use mw_core::{
    Actor, AuditFilter, Core, CoreError, LegacyStockState, NewStockItem, OrderPatch, StockFilter,
    StockImportSummary,
};
use mw_db::entity::{legacy_row, order, order_part};
use mw_domain::dictionary::DEFAULT_DICTIONARY;
use mw_domain::money::Money;
use mw_domain::permission::{Permission, PermissionSet};
use mw_legacy::fixture::FixtureVersion::V32;
use mw_legacy::{ImportReport, LegacyError, Warning, import_folder, migrate_stock};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder};
use tempfile::TempDir;

type Res = Result<(), Box<dyn std::error::Error>>;

/// База, импортированная из фикстуры V32 так, как импортировала программа до плана 8.1: склада
/// нет, «расходы» заказов — затраты MyWork 2 как есть, следы склада без цели, в отчёте
/// `stock_preserved`; файлы разложены, как у `import_folder`. Вложения — в папке `data`.
async fn import_like_before_stock(core: &Core, data: &Path) -> Result<ImportReport, LegacyError> {
    let src = fixture_dir(V32).await;
    mw_legacy::testing::import_before_stock(core, &admin(), &options(src.path(), data, false)).await
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
            },
            "{label}"
        );
        migrate_stock(&core, &admin()).await?;
        let after = core.legacy_stock_state(&admin()).await?;
        assert_eq!(
            (after.import_id, after.stock_rows, after.available),
            (Some(id), 4, false),
            "{label}"
        );

        // Склад уже заводили руками: перенос не предлагается, и сам перенос отказывает.
        let data = tempfile::tempdir()?;
        let core = with_data(&other.core, &data);
        import_like_before_stock(&core, data.path()).await?;
        core.create_stock_item(
            &admin(),
            NewStockItem {
                name: "Дисплей".into(),
                ..Default::default()
            },
        )
        .await?;
        let state = core.legacy_stock_state(&admin()).await?;
        assert_eq!((state.stock_rows, state.available), (4, false), "{label}");
        assert_eq!(
            field_error(&migrate_stock(&core, &admin()).await),
            stock_error("not_empty"),
            "{label}"
        );
    }
    Ok(())
}

/// Перенос даёт то же, что новый импорт той же папки: позиции, фото, запчасти, «расходы»,
/// справочники, цели следов и предупреждения отчёта.
#[tokio::test]
async fn migration_matches_fresh_import() -> Res {
    for (old, new) in cores().await.into_iter().zip(cores().await) {
        let label = old.label;
        let (old_data, new_data) = (tempfile::tempdir()?, tempfile::tempdir()?);
        let migrated = with_data(&old.core, &old_data);
        let fresh = with_data(&new.core, &new_data);
        let before = import_like_before_stock(&migrated, old_data.path()).await?;
        let src = fixture_dir(V32).await;
        let imported = import_folder(
            &fresh,
            &admin(),
            &options(src.path(), new_data.path(), false),
        )
        .await?;
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
                order_parts: 5,
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
        assert_eq!(
            migrated.get_order(&admin(), 2).await?.extra_costs,
            Money::from_kopecks(29_950)
        );

        for (kind, _) in DEFAULT_DICTIONARY {
            let values = |core: Core| async move {
                core.list_dictionary(*kind, true)
                    .await
                    .unwrap()
                    .into_iter()
                    .map(|i| (i.value, i.archived))
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                values(migrated.clone()).await,
                values(fresh.clone()).await,
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
            audit_summaries(&migrated, 2, "order.costs_split").await,
            ["Из расходов выделены запчасти со склада MyWork 2: было 4 000,00 ₽, стало 299,50 ₽"],
            "{label}"
        );
        assert_eq!(
            audit_summaries(&migrated, 5, "order.costs_split").await,
            ["Из расходов выделены запчасти со склада MyWork 2: было 500,00 ₽, стало 200,00 ₽"],
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
            (&serde_json::json!(4), &serde_json::json!(5)),
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
                && w.value == "позиций 4, запчастей 5"),
            "{label}"
        );
        assert_eq!(
            sorted(warnings),
            sorted(imported.warnings.clone()),
            "{label}"
        );
        let view = mw_legacy::report_view(&record, None);
        assert_eq!(view.stock_items, 4, "{label}");

        // Следующая позиция — после перенесённых.
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
        assert_eq!(next(migrated.clone()).await, 5, "{label}");
        assert_eq!(next(fresh.clone()).await, 5, "{label}");
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
        assert!(
            matches!(
                core.legacy_stock_state(&weak).await,
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
