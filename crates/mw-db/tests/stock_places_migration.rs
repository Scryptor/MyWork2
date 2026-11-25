//! Карта склада в схеме: шкафы и места позиций; внешние ключи и уникальность держит база.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use chrono::{NaiveDate, NaiveDateTime};
use mw_db::entity::{service_point, stock_item, stock_place, stock_rack};
use mw_db::testing::all_backends;
use sea_orm::{ActiveModelTrait, DatabaseConnection, DbErr, Set};

fn at() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 11, 25)
        .and_then(|d| d.and_hms_opt(10, 0, 0))
        .unwrap()
}

async fn item(db: &DatabaseConnection) -> i64 {
    stock_item::ActiveModel {
        part_type: Set("Дисплей".to_owned()),
        quantity: Set(3),
        created_at: Set(at()),
        updated_at: Set(at()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap()
    .id
}

async fn rack(
    db: &DatabaseConnection,
    code: &str,
    service_point_id: Option<i64>,
) -> Result<i64, DbErr> {
    Ok(stock_rack::ActiveModel {
        code: Set(code.to_owned()),
        shelves: Set(3),
        cells_per_shelf: Set(5),
        service_point_id: Set(service_point_id),
        ..Default::default()
    }
    .insert(db)
    .await?
    .id)
}

async fn place(
    db: &DatabaseConnection,
    item: i64,
    rack: i64,
    shelf: i32,
    cell: i32,
) -> Result<(), DbErr> {
    stock_place::ActiveModel {
        stock_item_id: Set(item),
        rack_id: Set(rack),
        shelf: Set(shelf),
        cell: Set(cell),
        quantity: Set(2),
        ..Default::default()
    }
    .insert(db)
    .await
    .map(|_| ())
}

#[tokio::test]
async fn places_need_a_live_item_and_rack() {
    for (label, db) in all_backends().await {
        let item = item(&db).await;
        let b = rack(&db, "B", None).await.unwrap();
        place(&db, item, b, 1, 1).await.unwrap();

        assert!(
            place(&db, 9999, b, 1, 2).await.is_err(),
            "{label}: позиции нет"
        );
        assert!(
            place(&db, item, 9999, 1, 2).await.is_err(),
            "{label}: шкафа нет"
        );
        assert!(
            place(&db, item, b, 1, 1).await.is_err(),
            "{label}: та же четвёрка"
        );
        place(&db, item, b, 1, 2).await.unwrap();

        assert!(rack(&db, "B", None).await.is_err(), "{label}: код занят");
        assert!(
            rack(&db, "C", Some(9999)).await.is_err(),
            "{label}: точки нет"
        );
        let point = service_point::ActiveModel {
            name: Set("Точка".to_owned()),
            sort_order: Set(0),
            archived: Set(false),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap()
        .id;
        assert!(rack(&db, "C", Some(point)).await.is_ok(), "{label}");
    }
}
