//! Позиции склада: добавление, проверки, поиск, «Докупить», сортировка, версии, права, история.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use common::{TestCore, actor_with, admin, assert_validation, cores};
use mw_core::{
    AuditFilter, CoreError, NewStockItem, PageRequest, StockFilter, StockItem, StockItemPatch,
    StockSort, StockSortField,
};
use mw_db::ids::{IdTable, set_next_id};
use mw_domain::money::Money;
use mw_domain::permission::Permission;
use mw_domain::stock::{MAX_STOCK_PRICE, MAX_STOCK_QUANTITY};
use serde_json::json;

async fn create(t: &TestCore, new: NewStockItem) -> StockItem {
    t.core.create_stock_item(&admin(), new).await.unwrap()
}

fn named(name: &str) -> NewStockItem {
    NewStockItem {
        name: name.into(),
        ..Default::default()
    }
}

/// Номера позиций списка по фильтру (страница до 50).
async fn listed(t: &TestCore, filter: StockFilter) -> Vec<i64> {
    t.core
        .list_stock(&admin(), &filter, 50, 0)
        .await
        .unwrap()
        .items
        .into_iter()
        .map(|i| i.id)
        .collect()
}

fn query(q: &str) -> StockFilter {
    StockFilter {
        query: q.into(),
        ..Default::default()
    }
}

fn sorted(field: StockSortField, descending: bool) -> StockFilter {
    StockFilter {
        sort: StockSort { field, descending },
        ..Default::default()
    }
}

#[tokio::test]
async fn created_item_gets_derived_name_and_restock_flag() -> Result<(), CoreError> {
    for t in cores().await {
        let it = t
            .core
            .create_stock_item(
                &admin(),
                NewStockItem {
                    part_type: " Дисплей ".into(),
                    device_type: "Телефон".into(),
                    model: "A52".into(),
                    quantity: 1,
                    restock_threshold: 2,
                    price: Money::from_rubles(2500),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(
            (
                it.name.as_str(),
                it.display_name.as_str(),
                it.part_type.as_str(),
                it.needs_restock,
                it.version
            ),
            ("", "Дисплей Телефон A52", "Дисплей", true, 1),
            "{}",
            t.label
        );
        assert_eq!(
            (it.quantity, it.price, it.restock_threshold),
            (1, Money::from_rubles(2500), 2)
        );
        assert_eq!((it.created_at, it.updated_at), (t.core.now(), t.core.now()));
        assert_eq!(t.core.get_stock_item(&admin(), it.id).await?, it);

        let journal = t
            .core
            .list_audit(
                &admin(),
                &AuditFilter {
                    stock_item_id: Some(it.id),
                    ..Default::default()
                },
                50,
                0,
            )
            .await?;
        assert_eq!(journal.total, 1, "{}", t.label);
        let entry = &journal.items[0];
        assert_eq!(
            (
                entry.action.as_str(),
                entry.stock_item_id,
                entry.order_id,
                entry.client_id
            ),
            ("stock.created", Some(it.id), None, None)
        );
        assert_eq!(
            entry.summary,
            "Добавлена позиция склада «Дисплей Телефон A52»"
        );
    }
    Ok(())
}

#[tokio::test]
async fn validation_codes() -> Result<(), CoreError> {
    for t in cores().await {
        let make = async |new: NewStockItem| t.core.create_stock_item(&admin(), new).await;
        assert_validation(make(NewStockItem::default()).await, &[("name", "required")]);
        // Примечание наименования не даёт.
        assert_validation(
            make(NewStockItem {
                name: "   ".into(),
                note: "Только примечание".into(),
                ..Default::default()
            })
            .await,
            &[("name", "required")],
        );
        for quantity in [-1, MAX_STOCK_QUANTITY + 1] {
            assert_validation(
                make(NewStockItem {
                    quantity,
                    ..named("Шлейф")
                })
                .await,
                &[("quantity", "out_of_range")],
            );
        }
        assert_validation(
            make(NewStockItem {
                restock_threshold: -1,
                ..named("Шлейф")
            })
            .await,
            &[("restock_threshold", "out_of_range")],
        );
        for price in [
            Money::from_kopecks(-1),
            Money::from_kopecks(MAX_STOCK_PRICE.kopecks() + 1),
        ] {
            assert_validation(
                make(NewStockItem {
                    price,
                    ..named("Шлейф")
                })
                .await,
                &[("price", "out_of_range")],
            );
        }
        assert_validation(
            make(NewStockItem {
                model: "м".repeat(201),
                ..named("Шлейф")
            })
            .await,
            &[("model", "too_long")],
        );
        assert_validation(
            make(NewStockItem {
                note: "п".repeat(2001),
                ..named("Шлейф")
            })
            .await,
            &[("note", "too_long")],
        );
        // Границы допустимы.
        let edge = make(NewStockItem {
            model: "м".repeat(200),
            note: "п".repeat(2000),
            quantity: MAX_STOCK_QUANTITY,
            restock_threshold: MAX_STOCK_QUANTITY,
            price: MAX_STOCK_PRICE,
            ..named("Шлейф")
        })
        .await?;
        assert_eq!(edge.quantity, MAX_STOCK_QUANTITY, "{}", t.label);

        // Правка проверяет то, что меняет.
        let it = create(&t, named("Кнопка")).await;
        assert_validation(
            t.core
                .update_stock_item(
                    &admin(),
                    it.id,
                    it.version,
                    StockItemPatch {
                        quantity: Some(-1),
                        ..Default::default()
                    },
                )
                .await,
            &[("quantity", "out_of_range")],
        );
        assert_validation(
            t.core
                .update_stock_item(
                    &admin(),
                    it.id,
                    it.version,
                    StockItemPatch {
                        name: Some(" ".into()),
                        ..Default::default()
                    },
                )
                .await,
            &[("name", "required")],
        );
        assert_eq!(t.core.get_stock_item(&admin(), it.id).await?, it);
    }
    Ok(())
}

#[tokio::test]
async fn search_ignores_case_and_yo() -> Result<(), CoreError> {
    for t in cores().await {
        let display = create(&t, named("ДИСПЛЕЙ SAMSUNG Чёрный")).await;
        let other = create(
            &t,
            NewStockItem {
                part_type: "Аккумулятор".into(),
                note: "Для Samsung".into(),
                ..Default::default()
            },
        )
        .await;
        assert_eq!(
            listed(&t, query("дисплей samsung черный")).await,
            vec![display.id],
            "{}",
            t.label
        );
        assert_eq!(listed(&t, query("чёрн")).await, vec![display.id]);
        // Примечание тоже ищется.
        assert_eq!(
            listed(&t, query("samsung")).await,
            vec![other.id, display.id]
        );

        set_next_id(t.core.db(), IdTable::StockItems, 41)
            .await
            .unwrap();
        let n41 = create(&t, named("Шлейф")).await;
        assert_eq!(n41.id, 41, "{}", t.label);
        let glass = create(
            &t,
            NewStockItem {
                part_type: "Стекло".into(),
                model: "A41".into(),
                ..Default::default()
            },
        )
        .await;
        // «41» — и номер позиции, и текст в модели другой позиции.
        assert_eq!(
            listed(&t, query(" 41 ")).await,
            vec![glass.id, n41.id],
            "{}",
            t.label
        );
        assert!(listed(&t, query("99")).await.is_empty());
    }
    Ok(())
}

#[tokio::test]
async fn restock_filter_and_summary() -> Result<(), CoreError> {
    for t in cores().await {
        let mut made = Vec::new();
        for (i, (quantity, restock_threshold)) in
            [(1, 5), (5, 5), (0, 0), (0, 3)].into_iter().enumerate()
        {
            made.push(
                create(
                    &t,
                    NewStockItem {
                        quantity,
                        restock_threshold,
                        ..named(&format!("Позиция {i}"))
                    },
                )
                .await,
            );
        }
        let deleted = &made[3];
        t.core
            .delete_stock_item(&admin(), deleted.id, deleted.version)
            .await?;
        let restock = StockFilter {
            restock: true,
            ..Default::default()
        };
        assert_eq!(listed(&t, restock).await, vec![made[0].id], "{}", t.label);
        assert_eq!(
            t.core
                .list_stock(&admin(), &StockFilter::default(), 50, 0)
                .await?
                .total,
            3
        );
        assert_eq!(t.core.stock_summary(&admin()).await?.restock, 1);
        assert_eq!(
            t.core
                .list_stock(&admin(), &StockFilter::default(), 50, 0)
                .await?
                .items
                .iter()
                .map(|i| i.needs_restock)
                .collect::<Vec<_>>(),
            [true, false, false]
        );
    }
    Ok(())
}

#[tokio::test]
async fn sort_by_name_uses_display_name() -> Result<(), CoreError> {
    for t in cores().await {
        let b = create(
            &t,
            NewStockItem {
                quantity: 1,
                price: Money::from_rubles(300),
                ..named("Б-имя")
            },
        )
        .await;
        let a = create(
            &t,
            NewStockItem {
                part_type: "Аккумулятор".into(),
                brand: "Zte".into(),
                quantity: 7,
                price: Money::from_rubles(100),
                ..Default::default()
            },
        )
        .await;
        let c = create(
            &t,
            NewStockItem {
                quantity: 3,
                price: Money::from_rubles(200),
                ..named("в-имя")
            },
        )
        .await;
        assert_eq!(a.display_name, "Аккумулятор Zte", "{}", t.label);
        assert_eq!(
            listed(&t, StockFilter::default()).await,
            vec![a.id, b.id, c.id],
            "{}",
            t.label
        );
        assert_eq!(
            listed(&t, sorted(StockSortField::Name, true)).await,
            vec![c.id, b.id, a.id]
        );
        assert_eq!(
            listed(&t, sorted(StockSortField::Id, false)).await,
            vec![b.id, a.id, c.id]
        );
        assert_eq!(
            listed(&t, sorted(StockSortField::Quantity, true)).await,
            vec![a.id, c.id, b.id]
        );
        assert_eq!(
            listed(&t, sorted(StockSortField::Price, false)).await,
            vec![a.id, c.id, b.id]
        );
        // Страница и общее число.
        let page = t
            .core
            .list_stock(&admin(), &StockFilter::default(), 1, 1)
            .await?;
        assert_eq!(
            (
                page.items.iter().map(|i| i.id).collect::<Vec<_>>(),
                page.total
            ),
            (vec![b.id], 3)
        );
        assert_validation(
            t.core
                .list_stock(&admin(), &StockFilter::default(), 0, 0)
                .await,
            &[("limit", "out_of_range")],
        );
    }
    Ok(())
}

#[tokio::test]
async fn stale_version_conflicts_and_delete_hides() -> Result<(), CoreError> {
    for t in cores().await {
        let it = create(&t, named("Дисплей")).await;
        let patch = |quantity: i64| StockItemPatch {
            quantity: Some(quantity),
            ..Default::default()
        };
        let edited = t
            .core
            .update_stock_item(&admin(), it.id, 1, patch(3))
            .await?;
        assert_eq!((edited.quantity, edited.version), (3, 2), "{}", t.label);
        assert!(
            matches!(
                t.core.update_stock_item(&admin(), it.id, 1, patch(4)).await,
                Err(CoreError::Conflict {
                    entity: "stock_item",
                    current_version: 2,
                    ..
                })
            ),
            "{}",
            t.label
        );
        assert!(matches!(
            t.core.delete_stock_item(&admin(), it.id, 1).await,
            Err(CoreError::Conflict {
                current_version: 2,
                ..
            })
        ));
        t.core.delete_stock_item(&admin(), it.id, 2).await?;
        assert!(listed(&t, StockFilter::default()).await.is_empty());
        assert!(matches!(
            t.core.get_stock_item(&admin(), it.id).await,
            Err(CoreError::NotFound {
                entity: "stock_item",
                ..
            })
        ));
        assert_validation(
            t.core.update_stock_item(&admin(), it.id, 3, patch(5)).await,
            &[("id", "deleted")],
        );
        assert_validation(
            t.core.delete_stock_item(&admin(), it.id, 3).await,
            &[("id", "deleted")],
        );
        assert!(matches!(
            t.core.get_stock_item(&admin(), 999).await,
            Err(CoreError::NotFound { id: 999, .. })
        ));
        assert!(matches!(
            t.core.update_stock_item(&admin(), 999, 1, patch(1)).await,
            Err(CoreError::NotFound { id: 999, .. })
        ));
        let journal = t
            .core
            .stock_item_history(&admin(), it.id, PageRequest::default())
            .await?;
        assert_eq!(
            journal
                .items
                .iter()
                .map(|e| (e.action.as_str(), e.summary.as_str(), e.stock_item_id))
                .collect::<Vec<_>>(),
            [
                (
                    "stock.deleted",
                    "Удалена позиция склада «Дисплей»",
                    Some(it.id)
                ),
                ("stock.updated", "Изменено: количество", Some(it.id)),
                (
                    "stock.created",
                    "Добавлена позиция склада «Дисплей»",
                    Some(it.id)
                ),
            ],
            "{}",
            t.label
        );
    }
    Ok(())
}

#[tokio::test]
async fn edit_journals_every_changed_field_with_its_label() -> Result<(), CoreError> {
    for t in cores().await {
        let it = create(
            &t,
            NewStockItem {
                part_type: "Дисплей".into(),
                quantity: 1,
                price: Money::from_rubles(100),
                ..Default::default()
            },
        )
        .await;
        t.clock.advance(chrono::Duration::minutes(5));
        let edited = t
            .core
            .update_stock_item(
                &admin(),
                it.id,
                it.version,
                StockItemPatch {
                    name: Some(" Экран ".into()),
                    device_type: Some("Телефон".into()),
                    part_type: Some("Дисплей".into()),
                    brand: Some("Samsung".into()),
                    model: Some("A52".into()),
                    color: Some("Чёрный".into()),
                    note: Some("Оригинал".into()),
                    quantity: Some(4),
                    price: Some(Money::from_rubles(150)),
                    restock_threshold: Some(2),
                },
            )
            .await?;
        assert_eq!(
            (
                edited.name.as_str(),
                edited.display_name.as_str(),
                edited.version,
                edited.needs_restock
            ),
            ("Экран", "Экран", 2, false),
            "{}",
            t.label
        );
        assert_eq!(edited.updated_at, t.core.now());
        assert_eq!(edited.created_at, it.created_at);
        let entry = t
            .core
            .stock_item_history(&admin(), it.id, PageRequest::default())
            .await?
            .items
            .remove(0);
        assert_eq!(
            entry.summary,
            "Изменено: наименование, тип устройства, бренд, модель, цвет, примечание, \
             количество, цена, докупить при",
            "{}",
            t.label
        );
        assert_eq!(entry.stock_item_id, Some(it.id));
        assert_eq!(
            entry.details.unwrap(),
            json!({ "changes": {
                "name": ["", "Экран"],
                "device_type": ["", "Телефон"],
                "brand": ["", "Samsung"],
                "model": ["", "A52"],
                "color": ["", "Чёрный"],
                "note": ["", "Оригинал"],
                "quantity": [1, 4],
                "price": [10000, 15000],
                "restock_threshold": [0, 2],
            } })
        );
        // Поиск и сортировка следуют за правкой.
        assert_eq!(listed(&t, query("экран чёрный")).await, vec![it.id]);
    }
    Ok(())
}

#[tokio::test]
async fn rights() -> Result<(), CoreError> {
    for t in cores().await {
        let it = create(&t, named("Дисплей")).await;
        let nobody = actor_with(&[]);
        assert!(matches!(
            t.core
                .list_stock(&nobody, &StockFilter::default(), 50, 0)
                .await,
            Err(CoreError::Forbidden(Permission::StockView))
        ));
        assert!(matches!(
            t.core.stock_summary(&nobody).await,
            Err(CoreError::Forbidden(Permission::StockView))
        ));
        assert!(matches!(
            t.core.get_stock_item(&nobody, it.id).await,
            Err(CoreError::Forbidden(Permission::StockView))
        ));
        assert!(matches!(
            t.core
                .stock_item_history(&nobody, it.id, PageRequest::default())
                .await,
            Err(CoreError::Forbidden(Permission::StockView))
        ));
        let viewer = actor_with(&[Permission::StockView, Permission::StockEdit]);
        assert!(matches!(
            t.core.create_stock_item(&viewer, named("Шлейф")).await,
            Err(CoreError::Forbidden(Permission::StockCreate))
        ));
        let creator = actor_with(&[Permission::StockView, Permission::StockCreate]);
        let patch = StockItemPatch {
            quantity: Some(2),
            ..Default::default()
        };
        assert!(matches!(
            t.core
                .update_stock_item(&creator, it.id, it.version, patch.clone())
                .await,
            Err(CoreError::Forbidden(Permission::StockEdit))
        ));
        assert!(matches!(
            t.core.delete_stock_item(&viewer, it.id, it.version).await,
            Err(CoreError::Forbidden(Permission::StockDelete))
        ));
        // Каждому — своё право.
        let made = t.core.create_stock_item(&creator, named("Шлейф")).await?;
        let edited = t
            .core
            .update_stock_item(&viewer, it.id, it.version, patch)
            .await?;
        t.core
            .delete_stock_item(
                &actor_with(&[Permission::StockDelete]),
                edited.id,
                edited.version,
            )
            .await?;
        assert_eq!(
            listed(&t, StockFilter::default()).await,
            vec![made.id],
            "{}",
            t.label
        );
    }
    Ok(())
}

#[tokio::test]
async fn history_lists_item_entries_with_stock_view_only() -> Result<(), CoreError> {
    for t in cores().await {
        let it = create(&t, named("Дисплей")).await;
        let other = create(&t, named("Шлейф")).await;
        t.core
            .update_stock_item(
                &admin(),
                it.id,
                it.version,
                StockItemPatch {
                    quantity: Some(3),
                    ..Default::default()
                },
            )
            .await?;
        let viewer = actor_with(&[Permission::StockView]);
        let history = t
            .core
            .stock_item_history(&viewer, it.id, PageRequest::default())
            .await?;
        assert_eq!(
            (
                history
                    .items
                    .iter()
                    .map(|e| e.action.as_str())
                    .collect::<Vec<_>>(),
                history.total
            ),
            (vec!["stock.updated", "stock.created"], 2),
            "{}",
            t.label
        );
        let page = t
            .core
            .stock_item_history(
                &viewer,
                it.id,
                PageRequest {
                    limit: 1,
                    offset: 1,
                },
            )
            .await?;
        assert_eq!(
            (page.items[0].action.as_str(), page.total),
            ("stock.created", 2)
        );
        let by_item = AuditFilter {
            stock_item_id: Some(it.id),
            ..Default::default()
        };
        assert_eq!(t.core.list_audit(&viewer, &by_item, 50, 0).await?.total, 2);
        assert_eq!(
            t.core
                .list_audit(
                    &viewer,
                    &AuditFilter {
                        stock_item_id: Some(other.id),
                        ..Default::default()
                    },
                    50,
                    0
                )
                .await?
                .total,
            1
        );
        assert!(matches!(
            t.core.list_audit(&actor_with(&[]), &by_item, 50, 0).await,
            Err(CoreError::Forbidden(Permission::StockView))
        ));
        assert!(matches!(
            t.core
                .list_audit(&viewer, &AuditFilter::default(), 50, 0)
                .await,
            Err(CoreError::Forbidden(Permission::AuditView))
        ));
        // Журнал целиком — с `audit_view`, и без права склада.
        assert_eq!(
            t.core
                .list_audit(&actor_with(&[Permission::AuditView]), &by_item, 50, 0)
                .await?
                .total,
            2
        );
        assert!(matches!(
            t.core
                .stock_item_history(&viewer, 999, PageRequest::default())
                .await,
            Err(CoreError::NotFound { id: 999, .. })
        ));
    }
    Ok(())
}
