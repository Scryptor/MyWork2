// Помощники интеграционных тестов вне `#[test]`-функций: unwrap здесь допустим.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use chrono::NaiveDate;
use mw_db::entity::{
    client, client_phone, dictionary_item, legacy_import, legacy_row, order, role, setting, status,
};
use mw_db::ids::IdTable;
use mw_domain::permission::PermissionSet;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, Set, Statement,
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

/// Начальные данные первой миграции заморожены литералами. Если константы `mw-domain` изменились,
/// этот тест падает: нужна НОВАЯ миграция данных, а не правка выпущенной.
#[tokio::test]
async fn seed_data_equals_current_domain_constants() {
    use mw_domain::dictionary::DEFAULT_DICTIONARY;
    use mw_domain::status::DEFAULT_STATUSES;
    for (label, db) in mw_db::testing::all_backends().await {
        let statuses = status::Entity::find()
            .order_by_asc(status::Column::SortOrder)
            .all(&db)
            .await
            .unwrap();
        let got: Vec<_> = statuses
            .iter()
            .map(|s| {
                (
                    s.name.as_str(),
                    s.kind.as_str(),
                    s.color.as_str(),
                    s.sort_order,
                )
            })
            .collect();
        let want: Vec<_> = (0_i32..)
            .zip(DEFAULT_STATUSES.iter())
            .map(|(i, s)| (s.name, s.kind.as_str(), s.color, i))
            .collect();
        assert_eq!(got, want, "{label}: статусы");

        let roles = role::Entity::find().all(&db).await.unwrap();
        assert_eq!(roles.len(), 1, "{label}");
        assert_eq!(
            roles[0].permissions,
            serde_json::to_string(&PermissionSet::all()).unwrap(),
            "{label}: права администратора"
        );

        for (kind, values) in DEFAULT_DICTIONARY {
            let items = dictionary_item::Entity::find()
                .filter(dictionary_item::Column::Kind.eq(kind.as_str()))
                .order_by_asc(dictionary_item::Column::SortOrder)
                .all(&db)
                .await
                .unwrap();
            let got: Vec<_> = items.iter().map(|i| i.value.as_str()).collect();
            assert_eq!(got, values.to_vec(), "{label}: справочник {kind:?}");
        }
        let total: usize = DEFAULT_DICTIONARY.iter().map(|(_, v)| v.len()).sum();
        assert_eq!(
            dictionary_item::Entity::find().count(&db).await.unwrap() as usize,
            total,
            "{label}"
        );
    }
}

/// Строка клиента только с колонками первой миграции.
async fn insert_client_row_with_phone(db: &DatabaseConnection, phone: &str) -> i64 {
    let key = mw_domain::phone::phone_key(phone);
    let model = client::ActiveModel {
        full_name: Set("Иванов Иван".to_owned()),
        phone: Set(phone.to_owned()),
        phone_key: Set(key),
        created_at: Set(now()),
        updated_at: Set(now()),
        ..Default::default()
    };
    client::Entity::insert(model)
        .exec(db)
        .await
        .unwrap()
        .last_insert_id
}

/// Строка заказа только с колонками первой миграции (новых колонок ещё нет).
async fn insert_order_row_with_warranty(
    db: &DatabaseConnection,
    client_id: i64,
    warranty_days: Option<i32>,
) -> i64 {
    let first_status = status::Entity::find()
        .order_by_asc(status::Column::SortOrder)
        .one(db)
        .await
        .unwrap()
        .unwrap();
    let backend = db.get_database_backend();
    let sql = "INSERT INTO orders (client_id, status_id, received_at, created_at, updated_at, warranty_days) \
               VALUES ($1, $2, $3, $3, $3, $4) RETURNING id";
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            sql,
            [
                client_id.into(),
                first_status.id.into(),
                now().into(),
                warranty_days.into(),
            ],
        ))
        .await
        .unwrap()
        .unwrap();
    row.try_get_by_index(0).unwrap()
}

async fn insert_legacy_import(db: &DatabaseConnection) -> i64 {
    let model = legacy_import::ActiveModel {
        imported_at: Set(now()),
        source_path: Set("C:/MyWork2".to_owned()),
        mode: Set("into_empty".to_owned()),
        report: Set("{}".to_owned()),
        ..Default::default()
    };
    model.insert(db).await.unwrap().id
}

async fn insert_legacy_row(
    db: &DatabaseConnection,
    import_id: i64,
    source: &str,
    key: &str,
) -> Result<(), DbErr> {
    let model = legacy_row::ActiveModel {
        import_id: Set(import_id),
        source: Set(source.to_owned()),
        source_key: Set(key.to_owned()),
        target_table: Set(None),
        target_id: Set(None),
        raw: Set("{}".to_owned()),
        ..Default::default()
    };
    legacy_row::Entity::insert(model).exec(db).await.map(|_| ())
}

#[tokio::test]
async fn legacy_import_migration_backfills_phones_and_warranty_text() {
    for (label, db) in mw_db::testing::all_backends_up_to(1).await {
        let client = insert_client_row_with_phone(&db, "79211234567").await;
        let _no_phone = insert_client_row_with_phone(&db, "").await;
        let thirty = insert_order_row_with_warranty(&db, client, Some(30)).await;
        let none = insert_order_row_with_warranty(&db, client, Some(0)).await;
        let unknown = insert_order_row_with_warranty(&db, client, None).await;
        mw_db::migrate(&db).await.unwrap();
        let phones = client_phone::Entity::find().all(&db).await.unwrap();
        assert_eq!(
            phones
                .iter()
                .map(|p| (
                    p.client_id,
                    p.position,
                    p.phone.as_str(),
                    p.phone_key.as_str(),
                    p.note.as_str()
                ))
                .collect::<Vec<_>>(),
            vec![(client, 0, "79211234567", "9211234567", "")],
            "{label}"
        );
        let text = |id: i64| {
            let db = db.clone();
            async move {
                order::Entity::find_by_id(id)
                    .one(&db)
                    .await
                    .unwrap()
                    .unwrap()
                    .warranty_text
            }
        };
        assert_eq!(text(thirty).await, "30 дн.", "{label}");
        assert_eq!(text(none).await, "Без гарантии", "{label}");
        assert_eq!(text(unknown).await, "", "{label}");
    }
}

#[tokio::test]
async fn legacy_rows_are_unique_per_source_row() {
    for (label, db) in mw_db::testing::all_backends().await {
        let import = insert_legacy_import(&db).await;
        insert_legacy_row(&db, import, "Catalog", "12")
            .await
            .unwrap();
        assert!(
            insert_legacy_row(&db, import, "Catalog", "12")
                .await
                .is_err(),
            "{label}"
        );
        assert!(
            insert_legacy_row(&db, import, "ClientsMap", "12")
                .await
                .is_ok(),
            "{label}"
        );
    }
}

#[tokio::test]
async fn vacuum_into_writes_a_consistent_copy_once() {
    let dir = tempfile::tempdir().unwrap();
    let db = mw_db::connect(&mw_db::sqlite_url(&dir.path().join("live.db")))
        .await
        .unwrap();
    mw_db::migrate(&db).await.unwrap();
    let copy = dir.path().join("backups").join("copy.db");
    mw_db::backup::vacuum_into(&db, &copy).await.unwrap();
    let restored = mw_db::connect(&mw_db::sqlite_url(&copy)).await.unwrap();
    assert_eq!(status::Entity::find().count(&restored).await.unwrap(), 8);
    assert!(mw_db::backup::vacuum_into(&db, &copy).await.is_err());
}

async fn put_setting_row(db: &DatabaseConnection, key: &str, value: &str) {
    setting::Entity::insert(setting::ActiveModel {
        key: Set(key.to_owned()),
        value: Set(value.to_owned()),
    })
    .exec_without_returning(db)
    .await
    .unwrap();
}

async fn setting_row(db: &DatabaseConnection, key: &str) -> Option<String> {
    setting::Entity::find_by_id(key)
        .one(db)
        .await
        .unwrap()
        .map(|m| m.value)
}

#[tokio::test]
async fn default_warranty_days_setting_becomes_text() {
    for (value, expected) in [
        ("45", "\"45 дн.\""),
        ("0", "\"Без гарантии\""),
        ("null", "\"\""),
    ] {
        for (label, db) in mw_db::testing::all_backends_up_to(2).await {
            put_setting_row(&db, "default_warranty_days", value).await;
            mw_db::migrate(&db).await.unwrap();
            assert_eq!(
                setting_row(&db, "default_warranty").await.as_deref(),
                Some(expected),
                "{label} {value}"
            );
            assert_eq!(
                setting_row(&db, "default_warranty_days").await,
                None,
                "{label}"
            );
        }
    }
}

#[tokio::test]
async fn missing_default_warranty_days_setting_adds_nothing() {
    for (label, db) in mw_db::testing::all_backends_up_to(2).await {
        mw_db::migrate(&db).await.unwrap();
        assert_eq!(setting_row(&db, "default_warranty").await, None, "{label}");
    }
}
