// Помощники интеграционных тестов вне `#[test]`-функций: unwrap здесь допустим.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use chrono::NaiveDate;
use mw_db::entity::{client, dictionary_item, order, role, status};
use mw_db::ids::IdTable;
use mw_domain::permission::PermissionSet;
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait, PaginatorTrait,
    QueryOrder, Set, Statement,
};

fn now() -> chrono::NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 11, 11)
        .and_then(|d| d.and_hms_opt(12, 0, 0))
        .unwrap_or_default()
}

async fn insert_client_row(db: &DatabaseConnection, id: Option<i64>) -> i64 {
    let model = client::ActiveModel {
        id: id.map_or(sea_orm::NotSet, Set),
        full_name: Set("Иванов Иван".to_owned()),
        created_at: Set(now()),
        updated_at: Set(now()),
        ..Default::default()
    };
    model.insert(db).await.unwrap().id
}

async fn insert_order_row_with_id(db: &DatabaseConnection, client_id: i64, id: i64) -> i64 {
    insert_order(db, client_id, Some(id)).await.unwrap()
}

async fn insert_order_row(db: &DatabaseConnection, client_id: i64) -> Result<i64, DbErr> {
    insert_order(db, client_id, None).await
}

async fn insert_order(
    db: &DatabaseConnection,
    client_id: i64,
    id: Option<i64>,
) -> Result<i64, DbErr> {
    let first_status = status::Entity::find()
        .order_by_asc(status::Column::SortOrder)
        .one(db)
        .await?
        .ok_or_else(|| DbErr::Custom("нет стартовых статусов".to_owned()))?;
    let model = order::ActiveModel {
        id: id.map_or(sea_orm::NotSet, Set),
        client_id: Set(client_id),
        status_id: Set(first_status.id),
        received_at: Set(now()),
        created_at: Set(now()),
        updated_at: Set(now()),
        ..Default::default()
    };
    Ok(model.insert(db).await?.id)
}

async fn pragma(db: &DatabaseConnection, name: &str) -> String {
    let row = db
        .query_one_raw(Statement::from_string(
            db.get_database_backend(),
            format!("PRAGMA {name}"),
        ))
        .await
        .unwrap()
        .unwrap();
    // Значение pragma бывает и числом (foreign_keys), и строкой (journal_mode).
    match row.try_get_by_index::<String>(0) {
        Ok(text) => text,
        Err(_) => row.try_get_by_index::<i64>(0).unwrap().to_string(),
    }
}

#[tokio::test]
async fn migration_creates_schema_and_seed_on_every_backend() {
    for (label, db) in mw_db::testing::all_backends().await {
        let statuses = status::Entity::find()
            .order_by_asc(status::Column::SortOrder)
            .all(&db)
            .await
            .unwrap();
        assert_eq!(
            statuses.iter().map(|s| s.name.as_str()).collect::<Vec<_>>()[7],
            "Выдан",
            "{label}"
        );
        assert_eq!(statuses.len(), 8, "{label}");
        let roles = role::Entity::find().all(&db).await.unwrap();
        assert_eq!(roles.len(), 1, "{label}");
        assert_eq!(roles[0].name, "Администратор");
        let perms: PermissionSet = serde_json::from_str(&roles[0].permissions).unwrap();
        assert_eq!(perms, PermissionSet::all());
        assert!(
            dictionary_item::Entity::find().count(&db).await.unwrap() > 40,
            "{label}"
        );
        mw_db::migrate(&db).await.unwrap(); // повторно — без ошибок
    }
}

#[tokio::test]
async fn foreign_keys_are_enforced_on_every_backend() {
    for (label, db) in mw_db::testing::all_backends().await {
        let res = insert_order_row(&db, /*client_id*/ 999).await;
        assert!(
            res.is_err(),
            "{label}: заказ с несуществующим клиентом должен отклоняться"
        );
    }
}

#[tokio::test]
async fn sqlite_file_database_uses_wal() {
    let dir = std::env::temp_dir().join(format!("mw-db-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = mw_db::connect(&mw_db::sqlite_url(&dir.join("t.db")))
        .await
        .unwrap();
    assert_eq!(pragma(&db, "journal_mode").await, "wal");
    assert_eq!(pragma(&db, "foreign_keys").await, "1");
}

#[tokio::test]
async fn set_next_id_continues_numbering_after_explicit_ids() {
    for (label, db) in mw_db::testing::all_backends().await {
        let client_id = insert_client_row(&db, Some(500)).await; // явный id, как при импорте
        insert_order_row_with_id(&db, client_id, 10_480).await;
        assert_eq!(
            mw_db::ids::max_id(&db, IdTable::Orders).await.unwrap(),
            10_480,
            "{label}"
        );
        mw_db::ids::set_next_id(&db, IdTable::Orders, 20_000)
            .await
            .unwrap();
        assert_eq!(
            insert_order_row(&db, client_id).await.unwrap(),
            20_000,
            "{label}"
        );
        mw_db::ids::set_next_id(&db, IdTable::Clients, 501)
            .await
            .unwrap();
        assert_eq!(insert_client_row(&db, None).await, 501, "{label}");
    }
}
