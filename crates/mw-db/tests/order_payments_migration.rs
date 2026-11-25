//! Платежи заказов: схема (внешний ключ, уникальный ключ повтора) и перенос старой предоплаты.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use chrono::{NaiveDate, NaiveDateTime};
use mw_db::entity::{client, order, order_payment, status};
use mw_db::testing::{all_backends, all_backends_up_to};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, NotSet, QueryFilter,
    QueryOrder, Set,
};

/// Миграций до платежей (схемы ещё нет).
const BEFORE_PAYMENTS: u32 = 20;
/// Миграций до переноса предоплаты (схема платежей уже есть).
const BEFORE_PAYMENTS_DATA: u32 = 21;

fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(y, m, d)
        .and_then(|d| d.and_hms_opt(h, min, 0))
        .unwrap()
}

async fn insert_client(db: &DatabaseConnection) -> i64 {
    client::ActiveModel {
        full_name: Set("Иванов Иван".to_owned()),
        created_at: Set(at(2015, 3, 1, 9, 0)),
        updated_at: Set(at(2015, 3, 1, 9, 0)),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap()
    .id
}

async fn insert_order(
    db: &DatabaseConnection,
    client_id: i64,
    received_at: NaiveDateTime,
    prepayment: i64,
    prepaid_at: Option<NaiveDateTime>,
    deleted_at: Option<NaiveDateTime>,
) -> i64 {
    let status = status::Entity::find()
        .order_by_asc(status::Column::SortOrder)
        .one(db)
        .await
        .unwrap()
        .unwrap()
        .id;
    order::ActiveModel {
        client_id: Set(client_id),
        status_id: Set(status),
        received_at: Set(received_at),
        prepayment: Set(prepayment),
        prepaid_at: Set(prepaid_at),
        deleted_at: Set(deleted_at),
        created_at: Set(received_at),
        updated_at: Set(received_at),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap()
    .id
}

async fn insert_payment(
    db: &DatabaseConnection,
    order_id: i64,
    amount: i64,
    request_key: Option<&str>,
) -> Result<order_payment::Model, DbErr> {
    order_payment::ActiveModel {
        id: NotSet,
        order_id: Set(order_id),
        day: Set(NaiveDate::from_ymd_opt(2026, 11, 20).unwrap()),
        amount: Set(amount),
        received_by: Set(String::new()),
        stage: Set("before_issue".to_owned()),
        added_by_id: Set(None),
        added_by_name: Set(String::new()),
        added_at: Set(at(2026, 11, 20, 10, 0)),
        legacy: Set(false),
        request_key: Set(request_key.map(str::to_owned)),
    }
    .insert(db)
    .await
}

async fn payments_of(db: &DatabaseConnection, order_id: i64) -> Vec<order_payment::Model> {
    order_payment::Entity::find()
        .filter(order_payment::Column::OrderId.eq(order_id))
        .order_by_asc(order_payment::Column::Id)
        .all(db)
        .await
        .unwrap()
}

#[tokio::test]
async fn prepayments_become_payments() {
    for (label, db) in all_backends_up_to(BEFORE_PAYMENTS).await {
        let client = insert_client(&db).await;
        let a = insert_order(
            &db,
            client,
            at(2015, 3, 4, 10, 0),
            50_000,
            Some(at(2015, 3, 5, 14, 20)),
            None,
        )
        .await;
        let b = insert_order(&db, client, at(2015, 3, 6, 9, 5), 30_000, None, None).await;
        let c = insert_order(&db, client, at(2015, 3, 6, 9, 6), 0, None, None).await;
        let d = insert_order(
            &db,
            client,
            at(2015, 3, 7, 8, 0),
            70_000,
            Some(at(2015, 3, 7, 11, 0)),
            Some(at(2015, 4, 1, 8, 0)),
        )
        .await;

        mw_db::migrate(&db).await.unwrap();

        for (order_id, day, amount, stamp) in [
            (a, (2015, 3, 5), 50_000, at(2015, 3, 5, 14, 20)),
            (b, (2015, 3, 6), 30_000, at(2015, 3, 6, 9, 5)),
            (d, (2015, 3, 7), 70_000, at(2015, 3, 7, 11, 0)),
        ] {
            let rows = payments_of(&db, order_id).await;
            assert_eq!(rows.len(), 1, "{label}: заказ {order_id}");
            let p = &rows[0];
            assert_eq!(
                p.day,
                NaiveDate::from_ymd_opt(day.0, day.1, day.2).unwrap(),
                "{label}"
            );
            assert_eq!((p.amount, p.added_at), (amount, stamp), "{label}");
            assert!(p.legacy, "{label}");
            assert_eq!(p.stage, "before_issue", "{label}");
            assert!(
                p.received_by.is_empty() && p.added_by_name.is_empty(),
                "{label}"
            );
            assert!(
                p.added_by_id.is_none() && p.request_key.is_none(),
                "{label}"
            );
        }
        assert!(payments_of(&db, c).await.is_empty(), "{label}");
        let prepayments: Vec<i64> = order::Entity::find()
            .order_by_asc(order::Column::Id)
            .all(&db)
            .await
            .unwrap()
            .into_iter()
            .map(|o| o.prepayment)
            .collect();
        assert_eq!(prepayments, [50_000, 30_000, 0, 70_000], "{label}");
    }
}

#[tokio::test]
async fn orders_with_payments_are_left_alone() {
    for (label, db) in all_backends_up_to(BEFORE_PAYMENTS_DATA).await {
        let client = insert_client(&db).await;
        let o = insert_order(&db, client, at(2026, 11, 19, 10, 0), 50_000, None, None).await;
        let existing = insert_payment(&db, o, 20_000, None).await.unwrap();

        mw_db::migrate(&db).await.unwrap();

        assert_eq!(payments_of(&db, o).await, [existing], "{label}");
    }
}

#[tokio::test]
async fn payments_need_an_order_and_a_unique_key() {
    for (label, db) in all_backends().await {
        let client = insert_client(&db).await;
        let first = insert_order(&db, client, at(2026, 11, 19, 10, 0), 0, None, None).await;
        let second = insert_order(&db, client, at(2026, 11, 19, 11, 0), 0, None, None).await;

        assert!(
            insert_payment(&db, 999, 100, None).await.is_err(),
            "{label}"
        );

        insert_payment(&db, first, 100, Some("k")).await.unwrap();
        assert!(
            insert_payment(&db, first, 200, Some("k")).await.is_err(),
            "{label}"
        );
        insert_payment(&db, second, 100, Some("k")).await.unwrap();
        insert_payment(&db, first, 100, None).await.unwrap();
        insert_payment(&db, first, 100, None).await.unwrap();
    }
}
