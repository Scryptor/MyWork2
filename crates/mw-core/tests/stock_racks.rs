//! Шкафы склада: создание, порядок, коды, проверки, карта (счётчики и «найдено»), занятые ячейки,
//! журнал и права.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use common::{TestCore, actor_with, admin, assert_validation, audit_details, cores, count_audit};
use mw_core::{
    CoreError, NewStockItem, NewStockRack, ServicePointPatch, StockFilter, StockRack,
    StockRackPatch,
};
use mw_db::entity::stock_place;
use mw_domain::permission::Permission;
use sea_orm::{ActiveModelTrait, EntityTrait, PaginatorTrait, Set};
use serde_json::json;

fn new_rack(code: &str, label: &str, shelves: i32, cells: i32) -> NewStockRack {
    NewStockRack {
        code: code.into(),
        label: label.into(),
        shelves,
        cells_per_shelf: cells,
        service_point_id: None,
    }
}

async fn make(t: &TestCore, new: NewStockRack) -> StockRack {
    t.core.create_stock_rack(&admin(), new).await.unwrap()
}

async fn list(t: &TestCore, filter: StockFilter) -> Vec<StockRack> {
    t.core.list_stock_racks(&admin(), &filter).await.unwrap()
}

async fn item(t: &TestCore, name: &str, quantity: i64, threshold: i64) -> i64 {
    t.core
        .create_stock_item(
            &admin(),
            NewStockItem {
                name: name.into(),
                quantity,
                restock_threshold: threshold,
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .id
}

async fn put(t: &TestCore, item: i64, rack: i64, shelf: i32, cell: i32) {
    stock_place::ActiveModel {
        stock_item_id: Set(item),
        rack_id: Set(rack),
        shelf: Set(shelf),
        cell: Set(cell),
        quantity: Set(1),
        ..Default::default()
    }
    .insert(t.core.db())
    .await
    .unwrap();
}

fn cell_codes(rack: &StockRack) -> Vec<&str> {
    rack.cells.iter().map(|c| c.code.as_str()).collect()
}

#[tokio::test]
async fn a_rack_is_created_listed_and_ordered() {
    for t in cores().await {
        let b = make(&t, new_rack(" b ", " Дисплеи ", 3, 5)).await;
        assert_eq!((b.code.as_str(), b.label.as_str()), ("B", "Дисплеи"));
        assert_eq!(b.cells.len(), 15, "{}", t.label);
        assert_eq!(b.cells[0].code, "B-1-1");
        assert_eq!(b.cells[14].code, "B-3-5");
        assert!(b.cells.iter().all(|c| c.positions == 0 && !c.found));
        let a = make(&t, new_rack("A", "", 1, 4)).await;
        assert_eq!(cell_codes(&a), ["A-1", "A-2", "A-3", "A-4"]);

        let order = |racks: &[StockRack]| racks.iter().map(|r| r.code.clone()).collect::<Vec<_>>();
        assert_eq!(order(&list(&t, StockFilter::default()).await), ["B", "A"]);
        t.core
            .reorder_stock_racks(&admin(), &[a.id, b.id], Some(&[b.id, a.id]))
            .await
            .unwrap();
        assert_eq!(order(&list(&t, StockFilter::default()).await), ["A", "B"]);
        assert_validation(
            t.core
                .reorder_stock_racks(&admin(), &[b.id, a.id], Some(&[b.id, a.id]))
                .await,
            &[("ordered_ids", "stale")],
        );
        assert_eq!(count_audit(&t.core, "stock_rack.created").await, 2);
        assert_eq!(count_audit(&t.core, "stock_racks.reordered").await, 1);
    }
}

#[tokio::test]
async fn rack_codes_are_unique_ignoring_case() {
    for t in cores().await {
        let b = make(&t, new_rack("B", "", 1, 1)).await;
        let a = make(&t, new_rack("A", "", 1, 1)).await;
        assert_validation(
            t.core
                .create_stock_rack(&admin(), new_rack("b", "", 1, 1))
                .await,
            &[("code", "duplicate")],
        );
        let patch = || StockRackPatch {
            code: Some("b".into()),
            ..Default::default()
        };
        assert_validation(
            t.core.update_stock_rack(&admin(), a.id, patch()).await,
            &[("code", "duplicate")],
        );
        let before = count_audit(&t.core, "stock_rack.updated").await;
        let same = t
            .core
            .update_stock_rack(&admin(), b.id, patch())
            .await
            .unwrap();
        assert_eq!(same.code, "B");
        assert_eq!(count_audit(&t.core, "stock_rack.updated").await, before);
    }
}

#[tokio::test]
async fn rack_checks_collect_every_error() {
    for t in cores().await {
        let point = t
            .core
            .create_service_point(&admin(), "Старая")
            .await
            .unwrap();
        t.core
            .update_service_point(
                &admin(),
                point.id,
                ServicePointPatch {
                    archived: Some(true),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let res = t
            .core
            .create_stock_rack(
                &admin(),
                NewStockRack {
                    code: "Б".into(),
                    label: "я".repeat(101),
                    shelves: 0,
                    cells_per_shelf: 21,
                    service_point_id: Some(point.id),
                },
            )
            .await;
        assert_validation(
            res,
            &[
                ("code", "rack_code"),
                ("shelves", "out_of_range"),
                ("cells_per_shelf", "out_of_range"),
                ("label", "too_long"),
                ("service_point_id", "invalid"),
            ],
        );
        let missing = NewStockRack {
            service_point_id: Some(9999),
            ..new_rack("C", "", 1, 1)
        };
        assert_validation(
            t.core.create_stock_rack(&admin(), missing).await,
            &[("service_point_id", "invalid")],
        );
        // Уже стоящая архивная точка в правке остаётся.
        let live = t
            .core
            .create_service_point(&admin(), "Живая")
            .await
            .unwrap();
        let rack = make(
            &t,
            NewStockRack {
                service_point_id: Some(live.id),
                ..new_rack("C", "", 1, 1)
            },
        )
        .await;
        t.core
            .update_service_point(
                &admin(),
                live.id,
                ServicePointPatch {
                    archived: Some(true),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let updated = t
            .core
            .update_stock_rack(
                &admin(),
                rack.id,
                StockRackPatch {
                    label: Some("Подпись".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(updated.service_point_id, Some(live.id));
    }
}

#[tokio::test]
async fn the_map_counts_and_marks_cells() {
    for t in cores().await {
        let b = make(&t, new_rack("B", "", 3, 3)).await;
        let display = item(&t, "Дисплей Samsung", 5, 0).await;
        let cable = item(&t, "Шлейф", 1, 3).await;
        put(&t, display, b.id, 1, 1).await;
        put(&t, cable, b.id, 1, 1).await;
        put(&t, cable, b.id, 2, 2).await;

        let found = |racks: &[StockRack]| {
            racks[0]
                .cells
                .iter()
                .filter(|c| c.found)
                .map(|c| c.code.clone())
                .collect::<Vec<_>>()
        };
        let racks = list(&t, StockFilter::default()).await;
        let positions: Vec<_> = racks[0].cells.iter().map(|c| c.positions).collect();
        assert_eq!(positions, [2, 0, 0, 0, 1, 0, 0, 0, 0], "{}", t.label);
        assert!(found(&racks).is_empty());

        let by_query = StockFilter {
            query: "samsung".into(),
            ..Default::default()
        };
        assert_eq!(found(&list(&t, by_query).await), ["B-1-1"]);
        let restock = StockFilter {
            restock: true,
            ..Default::default()
        };
        assert_eq!(found(&list(&t, restock).await), ["B-1-1", "B-2-2"]);
    }
}

#[tokio::test]
async fn shrinking_or_deleting_an_occupied_rack_changes_nothing() {
    for t in cores().await {
        let b = make(&t, new_rack("B", "", 3, 3)).await;
        let it = item(&t, "Шлейф", 5, 0).await;
        put(&t, it, b.id, 3, 2).await;
        put(&t, it, b.id, 3, 3).await;
        let journal = count_audit(&t.core, "stock_rack.updated").await
            + count_audit(&t.core, "stock_rack.deleted").await;
        let refusal = "В ячейках B-3-2, B-3-3 лежат запчасти — переложите их";
        let shrink = t
            .core
            .update_stock_rack(
                &admin(),
                b.id,
                StockRackPatch {
                    shelves: Some(2),
                    ..Default::default()
                },
            )
            .await;
        assert!(
            matches!(&shrink, Err(CoreError::InUse(m)) if m == refusal),
            "{shrink:?}"
        );
        let delete = t.core.delete_stock_rack(&admin(), b.id).await;
        assert!(
            matches!(&delete, Err(CoreError::InUse(m)) if m == refusal),
            "{delete:?}"
        );
        let racks = list(&t, StockFilter::default()).await;
        assert_eq!((racks.len(), racks[0].shelves), (1, 3));
        assert_eq!(
            stock_place::Entity::find()
                .count(t.core.db())
                .await
                .unwrap(),
            2
        );
        assert_eq!(
            count_audit(&t.core, "stock_rack.updated").await
                + count_audit(&t.core, "stock_rack.deleted").await,
            journal
        );

        for patch in [
            StockRackPatch {
                shelves: Some(4),
                ..Default::default()
            },
            StockRackPatch {
                cells_per_shelf: Some(3),
                ..Default::default()
            },
        ] {
            t.core
                .update_stock_rack(&admin(), b.id, patch)
                .await
                .unwrap();
        }
        stock_place::Entity::delete_many()
            .exec(t.core.db())
            .await
            .unwrap();
        t.core.delete_stock_rack(&admin(), b.id).await.unwrap();
        assert!(list(&t, StockFilter::default()).await.is_empty());
        assert_eq!(count_audit(&t.core, "stock_rack.deleted").await, 1);
        assert!(matches!(
            t.core.delete_stock_rack(&admin(), b.id).await,
            Err(CoreError::NotFound {
                entity: "шкаф", ..
            })
        ));
    }
}

#[tokio::test]
async fn refusal_names_at_most_ten_cells() {
    for t in cores().await {
        let b = make(&t, new_rack("B", "", 3, 4)).await;
        let it = item(&t, "Шлейф", 20, 0).await;
        for (shelf, cell) in (1..=3).flat_map(|s| (1..=4).map(move |c| (s, c))) {
            put(&t, it, b.id, shelf, cell).await;
        }
        let Err(CoreError::InUse(text)) = t.core.delete_stock_rack(&admin(), b.id).await else {
            panic!("ожидался отказ");
        };
        assert_eq!(
            text,
            "В ячейках B-1-1, B-1-2, B-1-3, B-1-4, B-2-1, B-2-2, B-2-3, B-2-4, B-3-1, B-3-2 и ещё 2 лежат запчасти — переложите их",
            "{}",
            t.label
        );
    }
}

#[tokio::test]
async fn codes_change_is_written_to_the_journal() {
    for t in cores().await {
        let b = make(&t, new_rack("B", "", 1, 4)).await;
        t.core
            .update_stock_rack(
                &admin(),
                b.id,
                StockRackPatch {
                    shelves: Some(2),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let details = audit_details(&t.core, "stock_rack.updated").await;
        assert_eq!(details.len(), 1);
        assert_eq!(details[0]["codes_changed"], json!(true), "{}", t.label);
        assert_eq!(details[0]["was"]["shelves"], json!(1));
        assert_eq!(details[0]["became"]["shelves"], json!(2));
        let summary = common::audit_json(&t.core, "stock_rack.updated").await;
        assert!(summary.starts_with("Изменён шкаф B"), "{summary}");
    }
}

#[tokio::test]
async fn racks_need_stock_rights() {
    for t in cores().await {
        let b = make(&t, new_rack("B", "", 1, 1)).await;
        let nobody = actor_with(&[]);
        let viewer = actor_with(&[Permission::StockView]);
        assert!(matches!(
            t.core
                .list_stock_racks(&nobody, &StockFilter::default())
                .await,
            Err(CoreError::Forbidden(Permission::StockView))
        ));
        assert!(
            t.core
                .list_stock_racks(&viewer, &StockFilter::default())
                .await
                .is_ok()
        );
        let denied = |r: Result<(), CoreError>| {
            assert!(
                matches!(r, Err(CoreError::Forbidden(Permission::StockEdit))),
                "{r:?}"
            );
        };
        assert!(matches!(
            t.core
                .create_stock_rack(&viewer, new_rack("C", "", 1, 1))
                .await,
            Err(CoreError::Forbidden(Permission::StockEdit))
        ));
        assert!(matches!(
            t.core
                .update_stock_rack(&viewer, b.id, StockRackPatch::default())
                .await,
            Err(CoreError::Forbidden(Permission::StockEdit))
        ));
        denied(t.core.delete_stock_rack(&viewer, b.id).await);
        denied(t.core.reorder_stock_racks(&viewer, &[b.id], None).await);
    }
}
