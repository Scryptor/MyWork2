//! Где лежит позиция: раскладка по ячейкам, согласование с остатком при правке, списании, возврате
//! и удалении, порядок кодов, параллельные записи.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use common::{
    TestCore, actor_with, admin, assert_validation, audit_details, cores, count_audit,
    sample_new_order,
};
use mw_core::{
    CoreError, NewOrderPart, NewStockItem, NewStockRack, PageRequest, StockCellRef, StockItem,
    StockItemPatch, StockMove, StockPut,
};
use mw_db::entity::stock_place;
use mw_domain::permission::Permission;
use sea_orm::{EntityTrait, PaginatorTrait};
use serde_json::json;

async fn rack(t: &TestCore, code: &str, shelves: i32, cells: i32) -> i64 {
    t.core
        .create_stock_rack(
            &admin(),
            NewStockRack {
                code: code.into(),
                label: String::new(),
                shelves,
                cells_per_shelf: cells,
                service_point_id: None,
            },
        )
        .await
        .unwrap()
        .id
}

async fn item(t: &TestCore, quantity: i64) -> StockItem {
    t.core
        .create_stock_item(
            &admin(),
            NewStockItem {
                name: "Дисплей".into(),
                quantity,
                ..Default::default()
            },
        )
        .await
        .unwrap()
}

fn at(rack_id: i64, shelf: i32, cell: i32) -> StockCellRef {
    StockCellRef {
        rack_id,
        shelf,
        cell,
    }
}

fn put(rack_id: i64, shelf: i32, cell: i32, quantity: i64) -> StockPut {
    StockPut {
        place: at(rack_id, shelf, cell),
        quantity,
    }
}

fn places(item: &StockItem) -> Vec<(String, i64)> {
    item.places
        .iter()
        .map(|p| (p.code.clone(), p.quantity))
        .collect()
}

fn pl(list: &[(&str, i64)]) -> Vec<(String, i64)> {
    list.iter().map(|&(c, q)| (c.to_owned(), q)).collect()
}

fn part(stock_item_id: i64, quantity: i64, key: &str) -> NewOrderPart {
    NewOrderPart {
        stock_item_id,
        quantity,
        request_key: key.into(),
    }
}

async fn get(t: &TestCore, id: i64) -> StockItem {
    t.core.get_stock_item(&admin(), id).await.unwrap()
}

async fn summaries(t: &TestCore, id: i64, action: &str) -> Vec<String> {
    let mut v: Vec<String> = t
        .core
        .stock_item_history(
            &admin(),
            id,
            PageRequest {
                limit: 100,
                offset: 0,
            },
        )
        .await
        .unwrap()
        .items
        .into_iter()
        .filter(|e| e.action == action)
        .map(|e| e.summary)
        .collect();
    v.reverse();
    v
}

#[tokio::test]
async fn put_move_and_unplace_keep_the_sum_within_the_quantity() {
    for t in cores().await {
        let a = rack(&t, "A", 1, 3).await;
        let b = rack(&t, "B", 3, 5).await;
        let it = item(&t, 10).await;
        assert_eq!((it.places.len(), it.unplaced), (0, 10));

        let r = t
            .core
            .put_stock(&admin(), it.id, put(b, 2, 4, 6))
            .await
            .unwrap();
        assert_eq!(places(&r), pl(&[("B-2-4", 6)]), "{}", t.label);
        assert_eq!((r.unplaced, r.version), (4, it.version + 1));
        let racks = t
            .core
            .list_stock_racks(&admin(), &Default::default())
            .await
            .unwrap();
        let cell = racks[1].cells.iter().find(|c| c.code == "B-2-4").unwrap();
        assert_eq!(cell.positions, 1);

        let r = t
            .core
            .move_stock(
                &admin(),
                it.id,
                StockMove {
                    from: at(b, 2, 4),
                    to: at(a, 1, 1),
                    quantity: 2,
                },
            )
            .await
            .unwrap();
        assert_eq!(places(&r), pl(&[("A-1", 2), ("B-2-4", 4)]));
        let r = t
            .core
            .move_stock(
                &admin(),
                it.id,
                StockMove {
                    from: at(a, 1, 1),
                    to: at(b, 2, 4),
                    quantity: 2,
                },
            )
            .await
            .unwrap();
        assert_eq!(places(&r), pl(&[("B-2-4", 6)]));
        assert_eq!(
            stock_place::Entity::find()
                .count(t.core.db())
                .await
                .unwrap(),
            1
        );
        let r = t
            .core
            .unplace_stock(&admin(), it.id, at(b, 2, 4))
            .await
            .unwrap();
        assert_eq!((r.places.len(), r.unplaced), (0, 10));

        assert_eq!(
            summaries(&t, it.id, "stock.placed").await,
            ["Положено в B-2-4: 6 шт."]
        );
        assert_eq!(
            summaries(&t, it.id, "stock.moved").await,
            [
                "Переложено из B-2-4 в A-1: 2 шт.",
                "Переложено из A-1 в B-2-4: 2 шт."
            ]
        );
        assert_eq!(
            summaries(&t, it.id, "stock.unplaced").await,
            ["Убрано из B-2-4: 6 шт."]
        );
    }
}

#[tokio::test]
async fn put_refuses_more_than_unplaced_and_unknown_cells() {
    for t in cores().await {
        let _a = rack(&t, "A", 1, 3).await;
        let b = rack(&t, "B", 3, 5).await;
        let it = item(&t, 10).await;
        let core = &t.core;
        assert_validation(
            core.put_stock(&admin(), it.id, put(b, 1, 1, 11)).await,
            &[("quantity", "places_exceed")],
        );
        for q in [0, -1] {
            assert_validation(
                core.put_stock(&admin(), it.id, put(b, 1, 1, q)).await,
                &[("quantity", "out_of_range")],
            );
        }
        assert_validation(
            core.put_stock(&admin(), it.id, put(b, 4, 1, 1)).await,
            &[("place", "no_such_place")],
        );
        assert_validation(
            core.put_stock(&admin(), it.id, put(9999, 1, 1, 1)).await,
            &[("place", "no_such_place")],
        );
        assert_validation(
            core.move_stock(
                &admin(),
                it.id,
                StockMove {
                    from: at(b, 1, 1),
                    to: at(b, 1, 2),
                    quantity: 1,
                },
            )
            .await,
            &[("quantity", "places_exceed")],
        );
        assert_validation(
            core.move_stock(
                &admin(),
                it.id,
                StockMove {
                    from: at(b, 9, 9),
                    to: at(b, 1, 2),
                    quantity: 1,
                },
            )
            .await,
            &[("from", "no_such_place")],
        );
        assert_validation(
            core.move_stock(
                &admin(),
                it.id,
                StockMove {
                    from: at(b, 1, 1),
                    to: at(b, 9, 9),
                    quantity: 1,
                },
            )
            .await,
            &[("to", "no_such_place")],
        );
        assert!(matches!(
            core.put_stock(&admin(), 9999, put(b, 1, 1, 1)).await,
            Err(CoreError::NotFound { .. })
        ));

        // Без изменений: ни версии, ни записи в журнале.
        let r = core
            .put_stock(&admin(), it.id, put(b, 1, 1, 4))
            .await
            .unwrap();
        let (version, entries) = (r.version, count_audit(core, "stock.moved").await);
        let same = core
            .move_stock(
                &admin(),
                it.id,
                StockMove {
                    from: at(b, 1, 1),
                    to: at(b, 1, 1),
                    quantity: 2,
                },
            )
            .await
            .unwrap();
        assert_eq!(same.version, version);
        let same = core
            .unplace_stock(&admin(), it.id, at(b, 2, 2))
            .await
            .unwrap();
        assert_eq!(same.version, version);
        assert_eq!(count_audit(core, "stock.moved").await, entries);
        assert_eq!(count_audit(core, "stock.unplaced").await, 0);

        let viewer = actor_with(&[Permission::StockView]);
        assert!(matches!(
            core.put_stock(&viewer, it.id, put(b, 1, 1, 1)).await,
            Err(CoreError::Forbidden(Permission::StockEdit))
        ));
        assert!(matches!(
            core.unplace_stock(&viewer, it.id, at(b, 1, 1)).await,
            Err(CoreError::Forbidden(Permission::StockEdit))
        ));
    }
}

#[tokio::test]
async fn a_new_item_goes_into_its_cell_at_once() {
    for t in cores().await {
        let b = rack(&t, "B", 3, 5).await;
        let new = |quantity, place| NewStockItem {
            name: "Шлейф".into(),
            quantity,
            place: Some(place),
            ..Default::default()
        };
        let it = t
            .core
            .create_stock_item(&admin(), new(5, at(b, 1, 1)))
            .await
            .unwrap();
        assert_eq!(places(&it), pl(&[("B-1-1", 5)]), "{}", t.label);
        assert_eq!(it.unplaced, 0);
        assert_eq!(get(&t, it.id).await.places, it.places);
        assert_eq!(count_audit(&t.core, "stock.created").await, 1);
        let details = audit_details(&t.core, "stock.created").await;
        assert_eq!(
            details[0]["placed"],
            json!({ "code": "B-1-1", "quantity": 5 })
        );

        let zero = t
            .core
            .create_stock_item(&admin(), new(0, at(b, 1, 1)))
            .await
            .unwrap();
        assert!(zero.places.is_empty());

        let before = t
            .core
            .list_stock(&admin(), &Default::default(), 50, 0)
            .await
            .unwrap()
            .total;
        assert_validation(
            t.core
                .create_stock_item(&admin(), new(3, at(b, 9, 9)))
                .await,
            &[("place", "no_such_place")],
        );
        let after = t
            .core
            .list_stock(&admin(), &Default::default(), 50, 0)
            .await
            .unwrap()
            .total;
        assert_eq!(before, after);
    }
}

#[tokio::test]
async fn write_off_takes_unplaced_first_then_cells_in_code_order() {
    for t in cores().await {
        let a = rack(&t, "A", 1, 3).await;
        let b = rack(&t, "B", 3, 5).await;
        let it = item(&t, 10).await;
        t.core
            .put_stock(&admin(), it.id, put(b, 1, 1, 3))
            .await
            .unwrap();
        t.core
            .put_stock(&admin(), it.id, put(a, 1, 2, 2))
            .await
            .unwrap();
        let o = t
            .core
            .create_order(&admin(), sample_new_order())
            .await
            .unwrap();

        t.core
            .add_order_part(&admin(), o.id, part(it.id, 4, "p1"))
            .await
            .unwrap();
        let now = get(&t, it.id).await;
        assert_eq!(now.quantity, 6);
        assert_eq!(places(&now), pl(&[("A-2", 2), ("B-1-1", 3)]), "{}", t.label);
        assert_eq!(now.unplaced, 1);
        let d = audit_details(&t.core, "order.part_added").await;
        assert!(d[0].get("places_reduced").is_none(), "{}", d[0]);

        t.core
            .add_order_part(&admin(), o.id, part(it.id, 2, "p2"))
            .await
            .unwrap();
        let now = get(&t, it.id).await;
        assert_eq!(places(&now), pl(&[("A-2", 1), ("B-1-1", 3)]));
        assert_eq!((now.quantity, now.unplaced), (4, 0));
        let d = audit_details(&t.core, "order.part_added").await;
        assert_eq!(
            d[1]["places_reduced"],
            json!([{ "code": "A-2", "quantity": 1 }])
        );
        let log = t
            .core
            .list_audit(&admin(), &Default::default(), 50, 0)
            .await
            .unwrap();
        let line = log
            .items
            .iter()
            .find(|e| e.action == "order.part_added" && e.summary.contains("(из ячеек"))
            .unwrap();
        assert!(
            line.summary.ends_with("(из ячеек: A-2 × 1)"),
            "{}",
            line.summary
        );

        t.core
            .add_order_part(&admin(), o.id, part(it.id, 3, "p3"))
            .await
            .unwrap();
        let now = get(&t, it.id).await;
        assert_eq!(places(&now), pl(&[("B-1-1", 1)]));
        assert_eq!((now.quantity, now.unplaced), (1, 0));
        let d = audit_details(&t.core, "order.part_added").await;
        assert_eq!(
            d[2]["places_reduced"],
            json!([{ "code": "A-2", "quantity": 1 }, { "code": "B-1-1", "quantity": 2 }])
        );

        // Возврат первой запчасти (4 шт.): места не трогаются.
        let parts = t.core.list_order_parts(&admin(), o.id).await.unwrap();
        t.core
            .return_order_part(&admin(), o.id, parts.parts[0].id)
            .await
            .unwrap();
        let now = get(&t, it.id).await;
        assert_eq!(now.quantity, 5);
        assert_eq!(places(&now), pl(&[("B-1-1", 1)]));
        assert_eq!(now.unplaced, 4);
    }
}

#[tokio::test]
async fn lowering_the_quantity_and_deleting_the_item_free_the_cells() {
    for t in cores().await {
        let a = rack(&t, "A", 1, 3).await;
        let b = rack(&t, "B", 3, 5).await;
        let it = item(&t, 10).await;
        t.core
            .put_stock(&admin(), it.id, put(a, 1, 1, 2))
            .await
            .unwrap();
        t.core
            .put_stock(&admin(), it.id, put(b, 1, 1, 3))
            .await
            .unwrap();
        let v = get(&t, it.id).await.version;
        let r = t
            .core
            .update_stock_item(
                &admin(),
                it.id,
                v,
                StockItemPatch {
                    quantity: Some(1),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(places(&r), pl(&[("B-1-1", 1)]), "{}", t.label);
        let d = audit_details(&t.core, "stock.updated").await;
        assert_eq!(
            d[0]["places_reduced"],
            json!([{ "code": "A-1", "quantity": 2 }, { "code": "B-1-1", "quantity": 2 }])
        );
        t.core.delete_stock_rack(&admin(), a).await.unwrap();

        t.core
            .delete_stock_item(&admin(), it.id, r.version)
            .await
            .unwrap();
        assert_eq!(
            stock_place::Entity::find()
                .count(t.core.db())
                .await
                .unwrap(),
            0
        );
        let d = audit_details(&t.core, "stock.deleted").await;
        assert_eq!(
            d[0]["places_reduced"],
            json!([{ "code": "B-1-1", "quantity": 1 }])
        );
    }
}

#[tokio::test]
async fn parallel_puts_never_place_more_than_the_quantity() {
    for t in cores().await {
        let a = rack(&t, "A", 1, 3).await;
        let b = rack(&t, "B", 3, 5).await;
        let it = item(&t, 5).await;
        t.core
            .put_stock(&admin(), it.id, put(a, 1, 1, 3))
            .await
            .unwrap();
        let adm = admin();
        let (x, y) = tokio::join!(
            t.core.put_stock(&adm, it.id, put(b, 1, 1, 2)),
            t.core.put_stock(&adm, it.id, put(b, 1, 2, 2)),
        );
        assert_eq!(
            [x.is_ok(), y.is_ok()].iter().filter(|&&ok| ok).count(),
            1,
            "{}",
            t.label
        );
        let failed = x.err().or(y.err()).unwrap();
        assert_validation::<()>(Err(failed), &[("quantity", "places_exceed")]);
        let now = get(&t, it.id).await;
        assert_eq!(now.places.iter().map(|p| p.quantity).sum::<i64>(), 5);

        let two = item(&t, 3).await;
        let o = t
            .core
            .create_order(&admin(), sample_new_order())
            .await
            .unwrap();
        let (_p, w1, w2) = tokio::join!(
            t.core.put_stock(&adm, two.id, put(b, 2, 1, 3)),
            t.core.add_order_part(&adm, o.id, part(two.id, 1, "w1")),
            t.core.add_order_part(&adm, o.id, part(two.id, 1, "w2")),
        );
        assert!(w1.is_ok() && w2.is_ok(), "{}", t.label);
        let now = get(&t, two.id).await;
        assert_eq!(now.quantity, 1);
        assert!(
            now.places.iter().map(|p| p.quantity).sum::<i64>() <= 1,
            "{now:?}"
        );
    }
}

#[tokio::test]
async fn places_come_in_code_order_on_every_backend() {
    for t in cores().await {
        let a10 = rack(&t, "A10", 1, 1).await;
        let a9 = rack(&t, "A9", 1, 1).await;
        let b = rack(&t, "B", 1, 12).await;
        let it = item(&t, 8).await;
        for (r, shelf, cell) in [(b, 1, 10), (a9, 1, 1), (b, 1, 2), (a10, 1, 1)] {
            t.core
                .put_stock(&admin(), it.id, put(r, shelf, cell, 2))
                .await
                .unwrap();
        }
        let now = get(&t, it.id).await;
        let codes: Vec<&str> = now.places.iter().map(|p| p.code.as_str()).collect();
        assert_eq!(codes, ["A10", "A9", "B-2", "B-10"], "{}", t.label);
        let o = t
            .core
            .create_order(&admin(), sample_new_order())
            .await
            .unwrap();
        // Всё разложено: снятие идёт сразу с первых мест по коду.
        t.core
            .add_order_part(&admin(), o.id, part(it.id, 1, "k1"))
            .await
            .unwrap();
        let now = get(&t, it.id).await;
        assert_eq!(
            places(&now),
            pl(&[("A10", 1), ("A9", 2), ("B-2", 2), ("B-10", 2)])
        );
    }
}

#[tokio::test]
async fn order_parts_show_places_to_who_sees_the_stock() {
    for t in cores().await {
        let b = rack(&t, "B", 3, 5).await;
        let it = item(&t, 5).await;
        t.core
            .put_stock(&admin(), it.id, put(b, 1, 1, 2))
            .await
            .unwrap();
        let o = t
            .core
            .create_order(&admin(), sample_new_order())
            .await
            .unwrap();
        t.core
            .add_order_part(&admin(), o.id, part(it.id, 1, "v1"))
            .await
            .unwrap();
        let seen = t.core.list_order_parts(&admin(), o.id).await.unwrap();
        assert_eq!(seen.parts[0].places, ["B-1-1"], "{}", t.label);
        let blind = actor_with(&[Permission::OrdersEdit]);
        let hidden = t.core.list_order_parts(&blind, o.id).await.unwrap();
        assert!(hidden.parts[0].places.is_empty());
    }
}
