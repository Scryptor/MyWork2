//! Склад в схеме: позиции с номерами из MyWork 2, запчасти заказов с ключом повтора, фото и журнал
//! позиции; готовые роли получают просмотр склада, только если их права не меняли.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use chrono::{NaiveDate, NaiveDateTime};
use mw_db::entity::{attachment, audit_log, client, order, order_part, status, stock_item};
use mw_db::ids::{IdTable, max_id, next_id, set_next_id};
use mw_db::testing::{all_backends, all_backends_up_to};
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait, NotSet, QueryOrder,
    Set, Statement,
};

/// Миграций до стартовых данных склада (схема склада уже есть).
const BEFORE_STOCK_DATA: u32 = 14;

fn at() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 11, 24)
        .and_then(|d| d.and_hms_opt(10, 0, 0))
        .unwrap()
}

async fn exec(db: &DatabaseConnection, sql: &str) {
    db.execute_raw(Statement::from_string(db.get_database_backend(), sql))
        .await
        .unwrap_or_else(|e| panic!("{sql}: {e}"));
}

async fn insert_item(db: &DatabaseConnection, id: Option<i64>, part_type: &str) -> i64 {
    stock_item::ActiveModel {
        id: id.map_or(NotSet, Set),
        part_type: Set(part_type.to_owned()),
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

async fn insert_order(db: &DatabaseConnection) -> i64 {
    let client = client::ActiveModel {
        full_name: Set("Иванов Иван".to_owned()),
        created_at: Set(at()),
        updated_at: Set(at()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap()
    .id;
    let status = status::Entity::find()
        .order_by_asc(status::Column::SortOrder)
        .one(db)
        .await
        .unwrap()
        .unwrap()
        .id;
    order::ActiveModel {
        client_id: Set(client),
        status_id: Set(status),
        received_at: Set(at()),
        created_at: Set(at()),
        updated_at: Set(at()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap()
    .id
}

async fn insert_part(
    db: &DatabaseConnection,
    order_id: i64,
    stock_item_id: Option<i64>,
    request_key: Option<&str>,
) -> Result<order_part::Model, DbErr> {
    order_part::ActiveModel {
        id: NotSet,
        order_id: Set(order_id),
        stock_item_id: Set(stock_item_id),
        name: Set("Дисплей Телефон A52".to_owned()),
        price: Set(250_000),
        quantity: Set(1),
        added_by_id: Set(None),
        added_by_name: Set("Мастер".to_owned()),
        added_at: Set(at()),
        legacy: Set(request_key.is_none()),
        request_key: Set(request_key.map(str::to_owned)),
    }
    .insert(db)
    .await
}

#[tokio::test]
async fn stock_tables_take_rows_and_ids_continue() {
    for (label, db) in all_backends().await {
        // Номер позиции из MyWork 2 сохраняется, а нумерация продолжается после него.
        assert_eq!(insert_item(&db, Some(40), "Дисплей").await, 40, "{label}");
        let next = max_id(&db, IdTable::StockItems).await.unwrap() + 1;
        set_next_id(&db, IdTable::StockItems, next).await.unwrap();
        assert_eq!(
            next_id(&db, IdTable::StockItems).await.unwrap(),
            41,
            "{label}"
        );
        assert_eq!(insert_item(&db, None, "Аккумулятор").await, 41, "{label}");
        let item = stock_item::Entity::find_by_id(41)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            (
                item.name.as_str(),
                item.price,
                item.restock_threshold,
                item.version,
                item.deleted_at
            ),
            ("", 0, 0, 1, None),
            "{label}: значения по умолчанию"
        );

        let order = insert_order(&db).await;
        // Перенесённые запчасти без ключа повтора — сколько угодно у одного заказа.
        insert_part(&db, order, Some(40), None).await.unwrap();
        insert_part(&db, order, None, None).await.unwrap();
        insert_part(&db, order, Some(41), Some("k1")).await.unwrap();
        assert!(
            insert_part(&db, order, Some(40), Some("k1")).await.is_err(),
            "{label}: тот же ключ у того же заказа"
        );
        let other = insert_order(&db).await;
        insert_part(&db, other, Some(40), Some("k1")).await.unwrap();
        assert!(
            insert_part(&db, 999_999, None, None).await.is_err(),
            "{label}: заказа нет"
        );
        assert!(
            insert_part(&db, order, Some(999_999), None).await.is_err(),
            "{label}: позиции нет"
        );
    }
}

#[tokio::test]
async fn photos_and_journal_entries_point_at_stock_items() {
    for (label, db) in all_backends().await {
        let item = insert_item(&db, None, "Дисплей").await;
        let photo = attachment::ActiveModel {
            stock_item_id: Set(Some(item)),
            kind: Set("photo".to_owned()),
            name: Set("1.jpg".to_owned()),
            content_type: Set("image/jpeg".to_owned()),
            size: Set(10),
            path: Set(format!("attachments/stock/{item}/1.jpg")),
            created_at: Set(at()),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
        assert_eq!(
            (photo.stock_item_id, photo.order_id),
            (Some(item), None),
            "{label}"
        );
        let entry = audit_log::ActiveModel {
            at: Set(at()),
            action: Set("stock.created".to_owned()),
            stock_item_id: Set(Some(item)),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
        assert_eq!(
            (entry.stock_item_id, entry.order_id),
            (Some(item), None),
            "{label}"
        );
    }
}

fn set(permissions: &[&str]) -> BTreeSet<String> {
    permissions.iter().map(|p| (*p).to_owned()).collect()
}

/// Права роли (разобранное множество) и её версия.
async fn role(db: &DatabaseConnection, name: &str) -> (BTreeSet<String>, i64) {
    let rows = db
        .query_all_raw(Statement::from_sql_and_values(
            db.get_database_backend(),
            "SELECT permissions, version FROM roles WHERE name = $1",
            [name.into()],
        ))
        .await
        .unwrap();
    assert_eq!(rows.len(), 1, "роль «{name}»");
    let permissions: String = rows[0].try_get_by_index(0).unwrap();
    (
        serde_json::from_str(&permissions).unwrap(),
        rows[0].try_get_by_index(1).unwrap(),
    )
}

#[tokio::test]
async fn builtin_roles_get_stock_view_only_when_untouched() {
    for (label, db) in all_backends_up_to(BEFORE_STOCK_DATA).await {
        // Права «Мастера» те же, только в другом порядке: сравнивается множество, а не строка.
        exec(
            &db,
            r#"UPDATE roles SET permissions = '["clients_view","orders_edit"]' WHERE name = 'Мастер'"#,
        )
        .await;
        exec(
            &db,
            r#"UPDATE roles SET permissions = '["orders_create"]' WHERE name = 'Приёмщик'"#,
        )
        .await;
        mw_db::migrate(&db).await.unwrap();
        assert_eq!(
            role(&db, "Мастер").await,
            (set(&["orders_edit", "stock_view", "clients_view"]), 2),
            "{label}"
        );
        assert_eq!(
            role(&db, "Приёмщик").await,
            (set(&["orders_create"]), 1),
            "{label}"
        );
        // Администратору склад не нужен — у него есть всё; версия 2 — от права «Выгрузка».
        let admin = role(&db, "Администратор").await;
        assert_eq!(admin.1, 2, "{label}: только право «Выгрузка»");
    }
}

/// Миграций до права «Выгрузка».
const BEFORE_DATA_EXPORT: u32 = 18;

const ALL_17: &str = r#"["orders_create","orders_edit","orders_delete","order_dates_edit","reports_view","stock_view","stock_create","stock_edit","stock_delete","clients_view","clients_create","clients_edit","clients_delete","clients_merge","settings_manage","users_manage","audit_view"]"#;

#[tokio::test]
async fn data_export_goes_to_roles_with_all_seventeen_rights() {
    for (label, db) in all_backends_up_to(BEFORE_DATA_EXPORT).await {
        // Администратор под другим именем: те же 17 прав в другом порядке.
        let shuffled = r#"["audit_view","orders_create","orders_edit","orders_delete","order_dates_edit","reports_view","stock_view","stock_create","stock_edit","stock_delete","clients_view","clients_create","clients_edit","clients_delete","clients_merge","settings_manage","users_manage"]"#;
        exec(
            &db,
            &format!("INSERT INTO roles (name, permissions, version) VALUES ('Директор', '{shuffled}', 1)"),
        )
        .await;
        let without_audit = ALL_17.replace(r#","audit_view""#, "");
        exec(
            &db,
            &format!("INSERT INTO roles (name, permissions, version) VALUES ('Без журнала', '{without_audit}', 1)"),
        )
        .await;
        mw_db::migrate(&db).await.unwrap();
        let mut all: BTreeSet<String> = serde_json::from_str(ALL_17).unwrap();
        all.insert("data_export".to_owned());
        assert_eq!(
            role(&db, "Администратор").await,
            (all.clone(), 2),
            "{label}"
        );
        assert_eq!(role(&db, "Директор").await, (all, 2), "{label}");
        let raw = db
            .query_one_raw(Statement::from_string(
                db.get_database_backend(),
                "SELECT permissions FROM roles WHERE name = 'Администратор'",
            ))
            .await
            .unwrap()
            .unwrap();
        let raw: String = raw.try_get_by_index(0).unwrap();
        assert!(
            raw.contains(r#""reports_view","data_export","stock_view""#),
            "{label}: {raw}"
        );
        for name in ["Мастер", "Приёмщик"] {
            assert!(
                !role(&db, name).await.0.contains("data_export"),
                "{label}: {name}"
            );
        }
        let (without, version) = role(&db, "Без журнала").await;
        assert!(!without.contains("data_export"), "{label}");
        assert_eq!(version, 1, "{label}");
    }
}
