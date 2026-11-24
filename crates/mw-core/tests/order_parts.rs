//! Запчасти заказа: списание со склада, возврат, повтор запроса, гонка за последнюю штуку,
//! версии заказа и позиции, удалённые заказ и позиция, права.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use common::{TestCore, actor_with, admin, assert_validation, cores, sample_new_order};
use mw_core::{
    Actor, AuditEntry, AuditFilter, CoreError, NewOrderPart, NewStockItem, Order, OrderParts,
    OrderPatch, SettingsPatch, StockItem, StockItemPatch,
};
use mw_db::entity::{order_part, stock_item};
use mw_domain::money::Money;
use mw_domain::permission::{Permission, PermissionSet};
use mw_domain::stock::MAX_STOCK_QUANTITY;
use sea_orm::{ActiveModelTrait, EntityTrait, PaginatorTrait, Set};

/// Позиция склада с наименованием, остатком и ценой в рублях.
async fn item(t: &TestCore, name: &str, quantity: i64, rubles: i64) -> StockItem {
    t.core
        .create_stock_item(
            &admin(),
            NewStockItem {
                name: name.into(),
                quantity,
                price: Money::from_rubles(rubles),
                ..Default::default()
            },
        )
        .await
        .unwrap()
}

async fn order(t: &TestCore) -> Order {
    t.core
        .create_order(&admin(), sample_new_order())
        .await
        .unwrap()
}

fn part(stock_item_id: i64, quantity: i64, key: &str) -> NewOrderPart {
    NewOrderPart {
        stock_item_id,
        quantity,
        request_key: key.into(),
    }
}

/// Позиция прямо из БД — и удалённая тоже.
async fn stored(t: &TestCore, id: i64) -> stock_item::Model {
    stock_item::Entity::find_by_id(id)
        .one(t.core.db())
        .await
        .unwrap()
        .unwrap()
}

/// Все строки запчастей всех заказов.
async fn part_rows(t: &TestCore) -> u64 {
    order_part::Entity::find().count(t.core.db()).await.unwrap()
}

/// Записи журнала позиции с этим действием, старые сверху.
async fn item_journal(t: &TestCore, id: i64, action: &str) -> Vec<AuditEntry> {
    let mut entries: Vec<AuditEntry> = t
        .core
        .list_audit(
            &admin(),
            &AuditFilter {
                stock_item_id: Some(id),
                ..Default::default()
            },
            50,
            0,
        )
        .await
        .unwrap()
        .items
        .into_iter()
        .filter(|e| e.action == action)
        .collect();
    entries.reverse();
    entries
}

fn not_found(res: Result<OrderParts, CoreError>, entity: &str, id: i64) {
    match res {
        Err(CoreError::NotFound { entity: e, id: i }) => {
            assert_eq!((e, i), (entity, id));
        }
        other => panic!("ожидалось «не найдено» {entity} №{id}, получено {other:?}"),
    }
}

fn forbidden(res: Result<OrderParts, CoreError>, permission: Permission) {
    match res {
        Err(CoreError::Forbidden(p)) => assert_eq!(p, permission),
        other => panic!("ожидался отказ в праве {permission:?}, получено {other:?}"),
    }
}

#[tokio::test]
async fn write_off_takes_from_stock_with_snapshot() -> Result<(), CoreError> {
    for t in cores().await {
        let it = item(&t, "Дисплей A52", 3, 2500).await;
        let o = order(&t).await;
        let parts = t
            .core
            .add_order_part(&admin(), o.id, part(it.id, 2, "first"))
            .await?;
        assert_eq!(parts.total, Money::from_rubles(5000), "{}", t.label);
        assert_eq!(parts.parts.len(), 1, "{}", t.label);
        let p = &parts.parts[0];
        assert_eq!(
            (
                p.stock_item_id,
                p.item_exists,
                p.name.as_str(),
                p.price,
                p.quantity,
                p.sum
            ),
            (
                Some(it.id),
                true,
                "Дисплей A52",
                Money::from_rubles(2500),
                2,
                Money::from_rubles(5000)
            ),
            "{}",
            t.label
        );
        assert_eq!(
            (p.added_by_name.as_str(), p.added_at, p.legacy),
            ("Тест", t.core.now(), false)
        );

        let after = t.core.get_stock_item(&admin(), it.id).await?;
        assert_eq!(
            (after.quantity, after.version),
            (1, it.version + 1),
            "{}",
            t.label
        );

        let journal = item_journal(&t, it.id, "order.part_added").await;
        assert_eq!(journal.len(), 1, "{}", t.label);
        let entry = &journal[0];
        assert_eq!(
            (entry.order_id, entry.client_id, entry.stock_item_id),
            (Some(o.id), Some(o.client_id), Some(it.id))
        );
        assert_eq!(
            entry.summary,
            "Списано со склада: Дисплей A52 × 2 — 5\u{a0}000\u{a0}₽"
        );

        // Снимок: правка позиции после списания строку заказа не меняет.
        t.core
            .update_stock_item(
                &admin(),
                it.id,
                after.version,
                StockItemPatch {
                    name: Some("Экран".into()),
                    price: Some(Money::from_rubles(3000)),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(t.core.list_order_parts(&admin(), o.id).await?, parts);
    }
    Ok(())
}

/// Суммы в журнале — со знаком валюты из настроек (без пробелов по краям), копейки — только если
/// они есть: тот же помощник, что у записи «Из расходов выделены …» переноса склада.
#[tokio::test]
async fn journal_sums_use_the_currency_setting() -> Result<(), CoreError> {
    for t in cores().await {
        t.core
            .update_settings(
                &admin(),
                SettingsPatch {
                    currency_symbol: Some(" € ".into()),
                    ..Default::default()
                },
            )
            .await?;
        let it = t
            .core
            .create_stock_item(
                &admin(),
                NewStockItem {
                    name: "Шлейф".into(),
                    quantity: 5,
                    price: Money::from_kopecks(123_455),
                    ..Default::default()
                },
            )
            .await?;
        let o = order(&t).await;
        let parts = t
            .core
            .add_order_part(&admin(), o.id, part(it.id, 2, "eur"))
            .await?;
        t.core
            .return_order_part(&admin(), o.id, parts.parts[0].id)
            .await?;
        let summaries = |action| {
            let t = &t;
            async move {
                item_journal(t, it.id, action)
                    .await
                    .into_iter()
                    .map(|e| e.summary)
                    .collect::<Vec<_>>()
            }
        };
        assert_eq!(
            summaries("order.part_added").await,
            ["Списано со склада: Шлейф × 2 — 2\u{a0}469,10\u{a0}€"],
            "{}",
            t.label
        );
        assert_eq!(
            summaries("order.part_returned").await,
            ["Возвращено на склад: Шлейф × 2 — 2\u{a0}469,10\u{a0}€"],
            "{}",
            t.label
        );
    }
    Ok(())
}

#[tokio::test]
async fn parts_are_listed_in_write_off_order_with_total() -> Result<(), CoreError> {
    for t in cores().await {
        let cable = item(&t, "Шлейф", 5, 300).await;
        let glass = item(&t, "Стекло", 5, 1000).await;
        let o = order(&t).await;
        assert_eq!(
            t.core.list_order_parts(&admin(), o.id).await?,
            OrderParts {
                parts: Vec::new(),
                total: Money::ZERO
            }
        );
        t.core
            .add_order_part(&admin(), o.id, part(glass.id, 1, "glass"))
            .await?;
        t.clock.advance(chrono::Duration::minutes(5));
        let parts = t
            .core
            .add_order_part(&admin(), o.id, part(cable.id, 3, "cable"))
            .await?;
        let names: Vec<&str> = parts.parts.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Стекло", "Шлейф"], "{}", t.label);
        assert_eq!(parts.total, Money::from_rubles(1900));
        assert_eq!(t.core.list_order_parts(&admin(), o.id).await?, parts);
        not_found(
            t.core.list_order_parts(&admin(), 999_999).await,
            "заказ",
            999_999,
        );
    }
    Ok(())
}

#[tokio::test]
async fn not_enough_stock_is_refused() -> Result<(), CoreError> {
    for t in cores().await {
        let it = item(&t, "Аккумулятор", 1, 900).await;
        let o = order(&t).await;
        assert_validation(
            t.core
                .add_order_part(&admin(), o.id, part(it.id, 2, "two"))
                .await,
            &[("quantity", "not_enough_stock")],
        );
        let after = t.core.get_stock_item(&admin(), it.id).await?;
        assert_eq!(
            (after.quantity, after.version),
            (1, it.version),
            "{}",
            t.label
        );
        assert_eq!(
            t.core.list_order_parts(&admin(), o.id).await?.parts.len(),
            0
        );
        assert_eq!(part_rows(&t).await, 0);
        assert!(item_journal(&t, it.id, "order.part_added").await.is_empty());
        // Отказ не занимает ключ: тот же ключ с доступным количеством проходит.
        t.core
            .add_order_part(&admin(), o.id, part(it.id, 1, "two"))
            .await?;
        assert_eq!(t.core.get_stock_item(&admin(), it.id).await?.quantity, 0);
    }
    Ok(())
}

#[tokio::test]
async fn last_unit_two_writers() -> Result<(), CoreError> {
    for t in cores().await {
        let it = item(&t, "Разъём зарядки", 1, 150).await;
        let (a, b) = (order(&t).await, order(&t).await);
        let actor = admin();
        let (ra, rb) = tokio::join!(
            t.core.add_order_part(&actor, a.id, part(it.id, 1, "tab-a")),
            t.core.add_order_part(&actor, b.id, part(it.id, 1, "tab-b")),
        );
        let (ok, refused) = match (ra, rb) {
            (Ok(ok), Err(e)) | (Err(e), Ok(ok)) => (ok, Err::<(), _>(e)),
            other => panic!("{}: ровно одно списание должно пройти: {other:?}", t.label),
        };
        assert_eq!(ok.parts.len(), 1);
        assert_validation(refused, &[("quantity", "not_enough_stock")]);
        let after = t.core.get_stock_item(&admin(), it.id).await?;
        assert_eq!((after.quantity, after.version), (0, it.version + 1));
        assert_eq!(part_rows(&t).await, 1, "{}", t.label);
    }
    Ok(())
}

#[tokio::test]
async fn same_request_key_writes_off_once() -> Result<(), CoreError> {
    for t in cores().await {
        let it = item(&t, "Динамик", 5, 400).await;
        let other = item(&t, "Микрофон", 5, 200).await;
        let o = order(&t).await;
        // Двойной щелчок: два одинаковых запроса одновременно.
        let actor = admin();
        let (first, retry) = tokio::join!(
            t.core.add_order_part(&actor, o.id, part(it.id, 1, "same")),
            t.core.add_order_part(&actor, o.id, part(it.id, 1, "same")),
        );
        let first = first?;
        assert_eq!(retry?, first, "{}", t.label);
        assert_eq!(first.parts.len(), 1);
        // Повтор после обрыва связи.
        assert_eq!(
            t.core
                .add_order_part(&admin(), o.id, part(it.id, 1, "same"))
                .await?,
            first
        );
        assert_eq!(t.core.get_stock_item(&admin(), it.id).await?.quantity, 4);
        assert_eq!(part_rows(&t).await, 1, "{}", t.label);
        assert_eq!(item_journal(&t, it.id, "order.part_added").await.len(), 1);

        // Тот же ключ с другим содержимым или от другого человека — отказ, остаток не меняется.
        for (actor, new) in [
            (admin(), part(it.id, 2, "same")),
            (admin(), part(other.id, 1, "same")),
            (
                Actor::for_tests(Some(7), "Другой", PermissionSet::all()),
                part(it.id, 1, "same"),
            ),
        ] {
            assert_validation(
                t.core.add_order_part(&actor, o.id, new).await,
                &[("request_key", "duplicate")],
            );
        }
        assert_eq!(t.core.get_stock_item(&admin(), it.id).await?.quantity, 4);
        assert_eq!(t.core.get_stock_item(&admin(), other.id).await?.quantity, 5);

        // Ключ уникален в пределах заказа.
        let o2 = order(&t).await;
        t.core
            .add_order_part(&admin(), o2.id, part(it.id, 1, "same"))
            .await?;
        assert_eq!(t.core.get_stock_item(&admin(), it.id).await?.quantity, 3);

        for key in ["", "с пробелом", &"x".repeat(65)] {
            assert_validation(
                t.core
                    .add_order_part(&admin(), o.id, part(it.id, 1, key))
                    .await,
                &[("request_key", "invalid")],
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn order_version_is_untouched() -> Result<(), CoreError> {
    for t in cores().await {
        let it = item(&t, "Камера", 2, 1200).await;
        let o = order(&t).await;
        let parts = t
            .core
            .add_order_part(&admin(), o.id, part(it.id, 1, "cam"))
            .await?;
        t.core
            .return_order_part(&admin(), o.id, parts.parts[0].id)
            .await?;
        t.core
            .add_order_part(&admin(), o.id, part(it.id, 1, "cam-2"))
            .await?;
        let now = t.core.get_order(&admin(), o.id).await?;
        assert_eq!(
            (now.version, now.updated_at),
            (o.version, o.updated_at),
            "{}",
            t.label
        );
        // Карточка с несохранённой правкой, открытая до списания, сохраняется без 409.
        let saved = t
            .core
            .update_order(
                &admin(),
                o.id,
                o.version,
                OrderPatch {
                    malfunction: Some("Не работает камера".into()),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(saved.version, o.version + 1);
    }
    Ok(())
}

#[tokio::test]
async fn stale_item_edit_after_write_off_conflicts() -> Result<(), CoreError> {
    for t in cores().await {
        let it = item(&t, "Корпус", 5, 700).await;
        let seen = t.core.get_stock_item(&admin(), it.id).await?;
        let o = order(&t).await;
        t.core
            .add_order_part(&admin(), o.id, part(it.id, 1, "body"))
            .await?;
        let res = t
            .core
            .update_stock_item(
                &admin(),
                it.id,
                seen.version,
                StockItemPatch {
                    quantity: Some(6),
                    ..Default::default()
                },
            )
            .await;
        match res {
            Err(CoreError::Conflict {
                current_version, ..
            }) => assert_eq!(current_version, seen.version + 1, "{}", t.label),
            other => panic!("{}: ожидался конфликт версий, получено {other:?}", t.label),
        }
        assert_eq!(t.core.get_stock_item(&admin(), it.id).await?.quantity, 4);
    }
    Ok(())
}

#[tokio::test]
async fn return_puts_back_or_just_removes() -> Result<(), CoreError> {
    for t in cores().await {
        let it = item(&t, "Шлейф", 5, 300).await;
        let o = order(&t).await;
        let added = t
            .core
            .add_order_part(&admin(), o.id, part(it.id, 2, "flex"))
            .await?;
        let part_id = added.parts[0].id;
        let before = t.core.get_stock_item(&admin(), it.id).await?;
        let left = t.core.return_order_part(&admin(), o.id, part_id).await?;
        assert_eq!(
            left,
            OrderParts {
                parts: Vec::new(),
                total: Money::ZERO
            },
            "{}",
            t.label
        );
        let after = t.core.get_stock_item(&admin(), it.id).await?;
        assert_eq!((after.quantity, after.version), (5, before.version + 1));
        let journal = item_journal(&t, it.id, "order.part_returned").await;
        assert_eq!(journal.len(), 1, "{}", t.label);
        assert_eq!(
            (
                journal[0].order_id,
                journal[0].client_id,
                journal[0].summary.as_str()
            ),
            (
                Some(o.id),
                Some(o.client_id),
                "Возвращено на склад: Шлейф × 2 — 600\u{a0}₽"
            )
        );
        // Повторный возврат той же строки — «не найдена».
        not_found(
            t.core.return_order_part(&admin(), o.id, part_id).await,
            "запчасть заказа",
            part_id,
        );
        assert_eq!(t.core.get_stock_item(&admin(), it.id).await?.quantity, 5);

        // Позицию удалили: строка снимается, остаток удалённой позиции не меняется.
        let gone = item(&t, "Кнопка", 3, 50).await;
        let added = t
            .core
            .add_order_part(&admin(), o.id, part(gone.id, 1, "button"))
            .await?;
        let seen = t.core.get_stock_item(&admin(), gone.id).await?;
        t.core
            .delete_stock_item(&admin(), gone.id, seen.version)
            .await?;
        let listed = t.core.list_order_parts(&admin(), o.id).await?;
        assert_eq!(
            (listed.parts[0].item_exists, listed.parts[0].name.as_str()),
            (false, "Кнопка"),
            "{}",
            t.label
        );
        let deleted = stored(&t, gone.id).await;
        t.core
            .return_order_part(&admin(), o.id, added.parts[0].id)
            .await?;
        assert_eq!(stored(&t, gone.id).await, deleted, "{}", t.label);
        assert_eq!(t.core.list_order_parts(&admin(), o.id).await?.parts, []);
        let journal = item_journal(&t, gone.id, "order.part_returned").await;
        assert_eq!(
            journal[0].summary,
            "Снято с заказа, позиции на складе нет: Кнопка × 1 — 50\u{a0}₽"
        );

        // Перенесённая из MyWork 2 строка без позиции снимается так же.
        let legacy = order_part::ActiveModel {
            order_id: Set(o.id),
            stock_item_id: Set(None),
            name: Set("Позиция №41 (нет на складе)".into()),
            price: Set(10_000),
            quantity: Set(1),
            added_by_id: Set(None),
            added_by_name: Set(String::new()),
            added_at: Set(t.core.now()),
            legacy: Set(true),
            request_key: Set(None),
            ..Default::default()
        }
        .insert(t.core.db())
        .await
        .unwrap();
        let listed = t.core.list_order_parts(&admin(), o.id).await?;
        assert_eq!(
            (
                listed.parts[0].stock_item_id,
                listed.parts[0].item_exists,
                listed.parts[0].legacy
            ),
            (None, false, true)
        );
        t.core.return_order_part(&admin(), o.id, legacy.id).await?;
        assert_eq!(part_rows(&t).await, 0, "{}", t.label);
    }
    Ok(())
}

#[tokio::test]
async fn deleted_order_and_item() -> Result<(), CoreError> {
    for t in cores().await {
        let it = item(&t, "Стекло", 5, 1000).await;

        // Удалённый заказ: ни списать, ни вернуть; список запчастей читается.
        let o = order(&t).await;
        let added = t
            .core
            .add_order_part(&admin(), o.id, part(it.id, 1, "glass"))
            .await?;
        t.core.delete_order(&admin(), o.id, o.version).await?;
        assert_validation(
            t.core
                .add_order_part(&admin(), o.id, part(it.id, 1, "glass-2"))
                .await,
            &[("id", "deleted")],
        );
        assert_validation(
            t.core
                .return_order_part(&admin(), o.id, added.parts[0].id)
                .await,
            &[("id", "deleted")],
        );
        assert_eq!(t.core.list_order_parts(&admin(), o.id).await?, added);
        assert_eq!(t.core.get_stock_item(&admin(), it.id).await?.quantity, 4);

        // Удалённая и несуществующая позиция.
        let live = order(&t).await;
        let gone = item(&t, "Плата", 2, 5000).await;
        t.core
            .delete_stock_item(&admin(), gone.id, gone.version)
            .await?;
        assert_validation(
            t.core
                .add_order_part(&admin(), live.id, part(gone.id, 1, "board"))
                .await,
            &[("stock_item_id", "deleted")],
        );
        assert_eq!(stored(&t, gone.id).await.quantity, 2, "{}", t.label);
        not_found(
            t.core
                .add_order_part(&admin(), live.id, part(999_999, 1, "nothing"))
                .await,
            "позиция склада",
            999_999,
        );
        not_found(
            t.core
                .add_order_part(&admin(), 999_999, part(it.id, 1, "no-order"))
                .await,
            "заказ",
            999_999,
        );

        // Количество 1…1 000 000.
        for quantity in [0, -1, MAX_STOCK_QUANTITY + 1] {
            assert_validation(
                t.core
                    .add_order_part(&admin(), live.id, part(it.id, quantity, "qty"))
                    .await,
                &[("quantity", "out_of_range")],
            );
        }

        // Строка другого заказа — «не найдена».
        let mine = t
            .core
            .add_order_part(&admin(), live.id, part(it.id, 1, "mine"))
            .await?;
        let stranger = order(&t).await;
        not_found(
            t.core
                .return_order_part(&admin(), stranger.id, mine.parts[0].id)
                .await,
            "запчасть заказа",
            mine.parts[0].id,
        );
        assert_eq!(t.core.list_order_parts(&admin(), live.id).await?, mine);
        assert_eq!(part_rows(&t).await, 2, "{}", t.label);
    }
    Ok(())
}

#[tokio::test]
async fn rights() -> Result<(), CoreError> {
    for t in cores().await {
        let it = item(&t, "Дисплей", 5, 2500).await;
        let o = order(&t).await;
        let orders_only = actor_with(&[Permission::OrdersEdit]);
        let stock_only = actor_with(&[Permission::StockView]);
        let nobody = actor_with(&[]);
        let master = actor_with(&[Permission::OrdersEdit, Permission::StockView]);

        forbidden(
            t.core
                .add_order_part(&orders_only, o.id, part(it.id, 1, "a"))
                .await,
            Permission::StockView,
        );
        forbidden(
            t.core
                .add_order_part(&stock_only, o.id, part(it.id, 1, "a"))
                .await,
            Permission::OrdersEdit,
        );
        let added = t
            .core
            .add_order_part(&master, o.id, part(it.id, 1, "a"))
            .await?;
        let part_id = added.parts[0].id;
        forbidden(
            t.core.return_order_part(&orders_only, o.id, part_id).await,
            Permission::StockView,
        );
        forbidden(
            t.core.return_order_part(&stock_only, o.id, part_id).await,
            Permission::OrdersEdit,
        );
        // Список запчастей — как сам заказ: без отдельного права.
        assert_eq!(t.core.list_order_parts(&nobody, o.id).await?, added);
        t.core.return_order_part(&master, o.id, part_id).await?;
        assert_eq!(t.core.get_stock_item(&admin(), it.id).await?.quantity, 5);
    }
    Ok(())
}
