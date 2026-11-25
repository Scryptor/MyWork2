//! Поиск склада по месту: код шкафа или ячейки в строке поиска, явный `place`, `found` у карты.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use common::{TestCore, admin, assert_validation, cores};
use mw_core::{NewStockItem, NewStockRack, StockFilter, StockListing, StockRack};
use mw_db::entity::stock_place;
use sea_orm::{ActiveModelTrait, Set};

struct Fixture {
    display: i64,
    cable: i64,
    case: i64,
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

async fn fixture(t: &TestCore) -> Fixture {
    let rack = t
        .core
        .create_stock_rack(
            &admin(),
            NewStockRack {
                code: "B".into(),
                label: "Дисплеи".into(),
                shelves: 3,
                cells_per_shelf: 5,
                service_point_id: None,
            },
        )
        .await
        .unwrap();
    let display = item(t, "Дисплей Samsung A51", 3, 5).await;
    let cable = item(t, "Шлейф", 3, 0).await;
    let case = item(t, "Корпус", 1, 0).await;
    put(t, display, rack.id, 2, 4).await;
    put(t, cable, rack.id, 1, 1).await;
    Fixture {
        display,
        cable,
        case,
    }
}

async fn search(t: &TestCore, filter: StockFilter) -> StockListing {
    t.core.search_stock(&admin(), &filter, 50, 0).await.unwrap()
}

fn query(q: &str) -> StockFilter {
    StockFilter {
        query: q.into(),
        ..Default::default()
    }
}

fn ids(l: &StockListing) -> Vec<i64> {
    let mut v: Vec<i64> = l.items.iter().map(|i| i.id).collect();
    v.sort_unstable();
    v
}

fn sorted(mut v: Vec<i64>) -> Vec<i64> {
    v.sort_unstable();
    v
}

#[tokio::test]
async fn a_query_equal_to_a_code_lists_what_lies_there() {
    for t in cores().await {
        let f = fixture(&t).await;
        let l = search(&t, query("b-2-4")).await;
        assert_eq!(
            (ids(&l), l.place.as_deref(), l.total),
            (vec![f.display], Some("B-2-4"), 1),
            "{}",
            t.label
        );
        let l = search(&t, query("B")).await;
        assert_eq!(
            (ids(&l), l.place.as_deref()),
            (sorted(vec![f.display, f.cable]), Some("B")),
            "{}",
            t.label
        );
        let l = search(&t, query("дисплей")).await;
        assert_eq!((ids(&l), l.place), (vec![f.display], None), "{}", t.label);
    }
}

#[tokio::test]
async fn a_scanned_code_in_the_russian_layout_finds_the_cell() {
    for t in cores().await {
        let f = fixture(&t).await;
        for q in ["И-2-4", "и-2-4"] {
            let l = search(&t, query(q)).await;
            assert_eq!(
                (ids(&l), l.place.as_deref()),
                (vec![f.display], Some("B-2-4")),
                "{}: {q}",
                t.label
            );
        }
    }
}

#[tokio::test]
async fn numbers_and_non_canonical_codes_stay_text() {
    for t in cores().await {
        let f = fixture(&t).await;
        let l = search(&t, query(&f.case.to_string())).await;
        assert_eq!((ids(&l), l.place), (vec![f.case], None), "{}", t.label);
        for q in ["B-02-4", "Z-1-1"] {
            let l = search(&t, query(q)).await;
            assert_eq!((l.items.len(), l.place), (0, None), "{}: {q}", t.label);
        }
    }
}

#[tokio::test]
async fn as_text_turns_the_place_off() {
    for t in cores().await {
        fixture(&t).await;
        let l = search(
            &t,
            StockFilter {
                query: "B-2-4".into(),
                as_text: true,
                ..Default::default()
            },
        )
        .await;
        assert_eq!((l.items.len(), l.place), (0, None), "{}", t.label);
    }
}

#[tokio::test]
async fn an_explicit_place_combines_with_words() {
    for t in cores().await {
        let f = fixture(&t).await;
        let place = |p: &str, q: &str, restock: bool| StockFilter {
            place: Some(p.into()),
            query: q.into(),
            restock,
            ..Default::default()
        };
        let l = search(&t, place("B", "шлейф", false)).await;
        assert_eq!(
            (ids(&l), l.place.as_deref()),
            (vec![f.cable], Some("B")),
            "{}",
            t.label
        );
        let l = search(&t, place("B", "", true)).await;
        assert_eq!(ids(&l), vec![f.display], "{}", t.label);
        let l = search(&t, place("b-2-4", "", true)).await;
        assert_eq!(
            (ids(&l), l.place.as_deref()),
            (vec![f.display], Some("B-2-4")),
            "{}",
            t.label
        );
        assert_validation(
            t.core
                .search_stock(&admin(), &place("Z-9", "", false), 50, 0)
                .await,
            &[("place", "no_such_place")],
        );
    }
}

#[tokio::test]
async fn the_map_marks_found_cells() {
    for t in cores().await {
        fixture(&t).await;
        let found = |racks: Vec<StockRack>| -> Vec<String> {
            racks
                .iter()
                .flat_map(|r| r.cells.iter())
                .filter(|c| c.found)
                .map(|c| c.code.clone())
                .collect()
        };
        let list =
            async |f: StockFilter| found(t.core.list_stock_racks(&admin(), &f).await.unwrap());
        assert_eq!(list(query("samsung")).await, vec!["B-2-4"], "{}", t.label);
        assert_eq!(list(query("b-1-1")).await, vec!["B-1-1"], "{}", t.label);
        assert!(list(StockFilter::default()).await.is_empty(), "{}", t.label);
    }
}
