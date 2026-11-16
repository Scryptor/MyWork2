//! Общие помощники тестов `mw-core`.
//!
//! A `// name` block in the plan's test listings that uses variables from an earlier block continues that block's test function; all other blocks are separate test functions with fresh `cores()`.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::fmt::Debug;
use std::sync::Arc;

use chrono::{TimeZone, Utc};
use mw_core::{
    Actor, ClientPhone, ClientRef, Core, CoreError, FixedClock, NewClient, NewOrder, OrderFilter,
    OrderPatch, OrderSort,
};
use mw_db::entity::{audit_log, client, order, role, session, setting, status, user};
use mw_domain::permission::Permission;
use mw_domain::status::StatusKind;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set,
};

pub struct TestCore {
    pub label: &'static str,
    pub core: Core,
    pub clock: Arc<FixedClock>,
}

/// По одному `Core` на бэкенд, пояс UTC, часы 2026-11-11T09:00:00Z.
pub async fn cores() -> Vec<TestCore> {
    let mut out = Vec::new();
    for (label, db) in mw_db::testing::all_backends().await {
        let clock = Arc::new(FixedClock::new(
            Utc.with_ymd_and_hms(2026, 11, 11, 9, 0, 0).unwrap(),
        ));
        let core = Core::open(db, clock.clone(), "UTC").await.unwrap();
        out.push(TestCore { label, core, clock });
    }
    out
}

pub fn admin() -> Actor {
    Actor::unrestricted("Тест")
}

pub fn actor_with(permissions: &[Permission]) -> Actor {
    let mut set = mw_domain::permission::PermissionSet::empty();
    for p in permissions {
        set.insert(*p);
    }
    Actor::for_tests(None, "Тест", set)
}

/// Пользователь с этим id и без прав, имя «Тест».
pub fn user_actor(user_id: i64) -> Actor {
    Actor::for_tests(
        Some(user_id),
        "Тест",
        mw_domain::permission::PermissionSet::empty(),
    )
}

/// Записывает вид статуса в обход `Core` (имитация повреждённых данных).
pub async fn set_status_kind_directly(t: &TestCore, status_id: i64, kind: &str) {
    status::ActiveModel {
        id: Set(status_id),
        kind: Set(kind.to_owned()),
        ..Default::default()
    }
    .update(t.core.db())
    .await
    .unwrap();
}

/// Ровно эти пары (поле, код) в этом порядке.
/// Телефоны без пометок.
pub fn phones(list: &[&str]) -> Vec<ClientPhone> {
    list.iter()
        .map(|p| ClientPhone {
            phone: (*p).into(),
            note: String::new(),
        })
        .collect()
}

pub fn assert_validation<T: Debug>(res: Result<T, CoreError>, expected: &[(&str, &str)]) {
    match res {
        Err(CoreError::Validation(fields)) => {
            let got: Vec<(&str, &str)> = fields
                .iter()
                .map(|f| (f.field.as_str(), f.code.as_str()))
                .collect();
            assert_eq!(got, expected);
        }
        other => panic!("ожидалась ошибка валидации {expected:?}, получено {other:?}"),
    }
}

pub trait HasId {
    fn id(&self) -> i64;
}

impl HasId for mw_core::Client {
    fn id(&self) -> i64 {
        self.id
    }
}

impl HasId for mw_core::OrderListItem {
    fn id(&self) -> i64 {
        self.id
    }
}

pub fn ids(items: impl IntoIterator<Item = impl HasId>) -> Vec<i64> {
    items.into_iter().map(|i| i.id()).collect()
}

pub async fn count_audit(core: &Core, action: &str) -> u64 {
    audit_log::Entity::find()
        .filter(audit_log::Column::Action.eq(action))
        .count(core.db())
        .await
        .unwrap()
}

/// Все сводки и details записей с этим кодом действия одной строкой.
pub async fn audit_json(core: &Core, action: &str) -> String {
    audit_log::Entity::find()
        .filter(audit_log::Column::Action.eq(action))
        .order_by_asc(audit_log::Column::Id)
        .all(core.db())
        .await
        .unwrap()
        .into_iter()
        .map(|r| format!("{} {}", r.summary, r.details.unwrap_or_default()))
        .collect::<Vec<_>>()
        .join("\n")
}

pub struct TestAudit {
    pub action: String,
    pub summary: String,
    pub details: Option<serde_json::Value>,
}

pub async fn last_audit(core: &Core, order_id: i64) -> TestAudit {
    let row = audit_log::Entity::find()
        .filter(audit_log::Column::OrderId.eq(order_id))
        .order_by_desc(audit_log::Column::Id)
        .one(core.db())
        .await
        .unwrap()
        .expect("нет записей журнала для заказа");
    TestAudit {
        action: row.action,
        summary: row.summary,
        details: row.details.map(|d| serde_json::from_str(&d).unwrap()),
    }
}

/// Сводки журнала заказа, старые сверху.
pub async fn audit_summaries(core: &Core, order_id: i64) -> Vec<String> {
    audit_log::Entity::find()
        .filter(audit_log::Column::OrderId.eq(order_id))
        .order_by_asc(audit_log::Column::Id)
        .all(core.db())
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.summary)
        .collect()
}

pub async fn status_id_by_name(core: &Core, name: &str) -> i64 {
    status::Entity::find()
        .filter(status::Column::Name.eq(name))
        .one(core.db())
        .await
        .unwrap()
        .unwrap_or_else(|| panic!("нет статуса {name}"))
        .id
}

pub async fn status_id_by_kind(core: &Core, kind: StatusKind) -> i64 {
    status::Entity::find()
        .filter(status::Column::Kind.eq(kind.as_str()))
        .order_by_asc(status::Column::SortOrder)
        .one(core.db())
        .await
        .unwrap()
        .unwrap_or_else(|| panic!("нет статуса вида {}", kind.as_str()))
        .id
}

pub async fn status_name(core: &Core, id: i64) -> String {
    status::Entity::find_by_id(id)
        .one(core.db())
        .await
        .unwrap()
        .unwrap_or_else(|| panic!("нет статуса {id}"))
        .name
}

pub async fn role_id_by_name(core: &Core, name: &str) -> i64 {
    role::Entity::find()
        .filter(role::Column::Name.eq(name))
        .one(core.db())
        .await
        .unwrap()
        .unwrap_or_else(|| panic!("нет роли {name}"))
        .id
}

pub fn sample_new_order() -> mw_core::NewOrder {
    mw_core::NewOrder {
        client: mw_core::ClientRef::New(mw_core::NewClient {
            full_name: "Пётр Иванов".into(),
            phones: phones(&["+7 921 123-45-67"]),
            ..Default::default()
        }),
        device_type: "Телефон".into(),
        brand: "Samsung".into(),
        model: "A52".into(),
        malfunction: "Разбит экран".into(),
        warranty: "30 дней".into(),
        ..Default::default()
    }
}

pub struct Seed {
    pub a: i64,
    pub b: i64,
    pub c: i64,
    pub d: i64,
    pub a_barcode: String,
}

fn at(day: u32, hour: u32) -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, day, hour, 0, 0).unwrap()
}

/// Четыре заказа для проверки списка; часы на время каждого приёма, в конце снова 2026-11-11T09:00:00Z.
/// A: диагностика, 06.11; B: «Готов», need_call, 10.11; C: выдан, 11.11 08:00; D: удалён, 11.11 09:00.
pub async fn seed_orders(t: &TestCore) -> Seed {
    let client = |name: &str, phone: &str| {
        ClientRef::New(NewClient {
            full_name: name.into(),
            phones: phones(&[phone]),
            ..Default::default()
        })
    };
    let a_order = |client, brand: &str, model: &str, device: &str| NewOrder {
        client,
        device_type: device.into(),
        brand: brand.into(),
        model: model.into(),
        malfunction: "Не включается".into(),
        ..Default::default()
    };
    let core = &t.core;

    t.clock.set(at(6, 9));
    let a = core
        .create_order(&admin(), sample_new_order())
        .await
        .unwrap();

    t.clock.set(at(10, 9));
    let b = core
        .create_order(
            &admin(),
            NewOrder {
                need_call: true,
                ..a_order(
                    client("Анна Ким", "8 911 000-00-00"),
                    "Apple",
                    "MacBook",
                    "Ноутбук",
                )
            },
        )
        .await
        .unwrap();
    let ready = status_id_by_name(core, "Готов").await;
    core.change_status(&admin(), b.id, 1, ready).await.unwrap();

    t.clock.set(at(11, 8));
    let c = core
        .create_order(
            &admin(),
            a_order(
                client("Олег Сидоров", "+7 999 555-44-33"),
                "Xiaomi",
                "Note 10",
                "Телефон",
            ),
        )
        .await
        .unwrap();
    core.update_order(
        &admin(),
        c.id,
        1,
        OrderPatch {
            master: Some("Мастер".into()),
            work_done: Some("Заменён экран".into()),
            final_price: Some(Some(mw_domain::money::Money::from_kopecks(500_000))),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    core.issue_order(&admin(), c.id, 2).await.unwrap();

    t.clock.set(at(11, 9));
    let d = core
        .create_order(
            &admin(),
            a_order(
                client("Иван Удалённый", "+7 900 000-00-01"),
                "Nokia",
                "3310",
                "Телефон",
            ),
        )
        .await
        .unwrap();
    core.delete_order(&admin(), d.id, 1).await.unwrap();

    Seed {
        a: a.id,
        b: b.id,
        c: c.id,
        d: d.id,
        a_barcode: a.barcode,
    }
}

/// Идентификаторы заказов по фильтру: сортировка по умолчанию, не больше 50.
pub async fn list_ids(t: &TestCore, filter: OrderFilter) -> Vec<i64> {
    ids(t
        .core
        .list_orders(&admin(), &filter, OrderSort::default(), 50, 0)
        .await
        .unwrap()
        .items)
}

/// Записывает штрихкод в обход `Core` (имитация старых кодов-дублей).
pub async fn set_barcode_directly(t: &TestCore, order_id: i64, code: &str) {
    order::ActiveModel {
        id: Set(order_id),
        barcode: Set(code.to_owned()),
        ..Default::default()
    }
    .update(t.core.db())
    .await
    .unwrap();
}

/// `token_hash` всех сессий.
pub async fn stored_token_hashes(core: &Core) -> Vec<String> {
    session::Entity::find()
        .all(core.db())
        .await
        .unwrap()
        .into_iter()
        .map(|s| s.token_hash)
        .collect()
}

/// Активный пользователь со старым хэшем MyWork 2 (SHA-1), в обход `Core`.
pub async fn insert_legacy_user(core: &Core, login: &str, sha1_hex: &str, role_id: i64) {
    let now = core.now();
    user::ActiveModel {
        login: Set(login.to_owned()),
        login_key: Set(mw_domain::search::normalize_search(login)),
        display_name: Set(String::new()),
        role_id: Set(role_id),
        password_hash: Set(sha1_hex.to_owned()),
        password_scheme: Set("legacy_sha1".to_owned()),
        active: Set(true),
        created_at: Set(now),
        updated_at: Set(now),
        version: Set(1),
        ..Default::default()
    }
    .insert(core.db())
    .await
    .unwrap();
}

/// Схема пароля пользователя (`argon2` / `legacy_sha1`).
pub async fn user_scheme(core: &Core, login: &str) -> String {
    user::Entity::find()
        .filter(user::Column::Login.eq(login))
        .one(core.db())
        .await
        .unwrap()
        .unwrap_or_else(|| panic!("нет пользователя {login}"))
        .password_scheme
}

/// Пользователь в обход `Core`: схема `argon2`, хэш-заглушка, пустое имя, версия 1.
pub async fn insert_user_row(core: &Core, login: &str, role_id: i64, active: bool) -> i64 {
    let now = core.now();
    user::ActiveModel {
        login: Set(login.to_owned()),
        login_key: Set(mw_domain::search::normalize_search(login)),
        display_name: Set(String::new()),
        role_id: Set(role_id),
        password_hash: Set("placeholder-not-a-real-hash".to_owned()),
        password_scheme: Set("argon2".to_owned()),
        active: Set(active),
        created_at: Set(now),
        updated_at: Set(now),
        version: Set(1),
        ..Default::default()
    }
    .insert(core.db())
    .await
    .unwrap()
    .id
}

/// `details` всех записей журнала с этим кодом действия, старые первыми (`Null` — без details).
pub async fn audit_details(core: &Core, action: &str) -> Vec<serde_json::Value> {
    audit_log::Entity::find()
        .filter(audit_log::Column::Action.eq(action))
        .order_by_asc(audit_log::Column::Id)
        .all(core.db())
        .await
        .unwrap()
        .into_iter()
        .map(|r| {
            r.details.map_or(serde_json::Value::Null, |d| {
                serde_json::from_str(&d).unwrap()
            })
        })
        .collect()
}

/// Записывает `search_text` клиента в обход `Core` (имитация устаревшего индекса).
pub async fn set_search_text_directly(core: &Core, client_id: i64, text: &str) {
    client::ActiveModel {
        id: Set(client_id),
        search_text: Set(text.to_owned()),
        ..Default::default()
    }
    .update(core.db())
    .await
    .unwrap();
}

/// Записывает настройку в обход `Core`.
pub async fn set_setting_directly(core: &Core, key: &str, value: serde_json::Value) {
    let row = setting::ActiveModel {
        key: Set(key.to_owned()),
        value: Set(value.to_string()),
    };
    setting::Entity::insert(row)
        .on_conflict(
            sea_orm::sea_query::OnConflict::column(setting::Column::Key)
                .update_column(setting::Column::Value)
                .to_owned(),
        )
        .exec_without_returning(core.db())
        .await
        .unwrap();
}

/// Значение настройки как JSON.
pub async fn setting_value(core: &Core, key: &str) -> serde_json::Value {
    let row = setting::Entity::find_by_id(key)
        .one(core.db())
        .await
        .unwrap()
        .unwrap();
    serde_json::from_str(&row.value).unwrap()
}

/// Пакет импорта из описания плана: 9 статусов, 2 точки, роли, 2 пользователя, 2 клиента, 3 заказа.
pub fn sample_bundle() -> mw_core::ImportBundle {
    use chrono::{NaiveDate, NaiveDateTime};
    use mw_core::{
        ImportBundle, ImportedClient, ImportedHistory, ImportedOrder, ImportedSettings,
        ImportedUser, LegacyTarget, NewRole, NewStatus, SettingsPatch,
    };
    use mw_domain::dictionary::{DEFAULT_DICTIONARY, DictionaryKind};
    use mw_domain::money::Money;
    use mw_domain::permission::PermissionSet;
    use mw_domain::status::DEFAULT_STATUSES;
    use serde_json::json;

    let dt = |y, mo, d, h, mi| -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, mo, d)
            .unwrap()
            .and_hms_opt(h, mi, 0)
            .unwrap()
    };
    let mut statuses: Vec<NewStatus> = DEFAULT_STATUSES
        .iter()
        .map(|s| NewStatus {
            name: s.name.to_owned(),
            kind: s.kind,
            color: s.color.to_owned(),
        })
        .collect();
    statuses.push(NewStatus {
        name: "Куплен".into(),
        kind: StatusKind::Issued,
        color: "gray".into(),
    });
    let dictionary: Vec<(DictionaryKind, Vec<String>)> = DEFAULT_DICTIONARY
        .iter()
        .map(|(kind, values)| {
            let values = if *kind == DictionaryKind::Master {
                vec!["ПЕТРОВ".to_owned()]
            } else {
                values.iter().map(|v| (*v).to_owned()).collect()
            };
            (*kind, values)
        })
        .collect();
    let order = |id: i64, client_id: i64, status: &str, received_at: NaiveDateTime| ImportedOrder {
        id,
        client_id,
        status: status.to_owned(),
        service_point: None,
        received_at,
        issued_at: None,
        prepaid_at: None,
        device_type: "ТЕЛЕФОН".into(),
        brand: "SAMSUNG".into(),
        model: "A52".into(),
        serial: String::new(),
        color: String::new(),
        completeness: String::new(),
        device_condition: String::new(),
        malfunction: "Разбит экран".into(),
        work_done: String::new(),
        internal_comment: String::new(),
        master: "ПЕТРОВ".into(),
        barcode: format!("{id:012}"),
        estimate_from: None,
        estimate_to: None,
        final_price: None,
        prepayment: Money::ZERO,
        extra_costs: Money::ZERO,
        discount: Money::ZERO,
        warranty: String::new(),
        need_call: false,
        need_parts: false,
        deleted_at: None,
        tracking: Vec::new(),
    };
    let mut o5 = order(5, 7, "Выдан", dt(2015, 3, 5, 14, 20));
    o5.issued_at = Some(dt(2015, 3, 7, 0, 0));
    o5.final_price = Some(Money::from_rubles(1500));
    o5.warranty = "30 дней".into();
    o5.barcode = "123456789012".into();
    let mut o6 = order(6, 7, "Куплен", dt(2015, 3, 6, 9, 5));
    o6.deleted_at = Some(dt(2015, 3, 8, 10, 0));
    o6.barcode = "123456789012".into();
    let mut o10 = order(10, 9, "Диагностика", dt(2015, 3, 17, 12, 0));
    o10.service_point = Some("Ленина, 1".into());
    o10.estimate_from = Some(Money::from_rubles(1500));
    o10.estimate_to = Some(Money::from_rubles(1500));
    o10.tracking = vec![("RA123456789RU".into(), "посылка".into())];
    let client = |id: i64, name: &str, phones: Vec<ClientPhone>, created_at| ImportedClient {
        id,
        full_name: name.to_owned(),
        phones,
        address: String::new(),
        note: String::new(),
        source: String::new(),
        blacklisted: false,
        created_at,
    };
    ImportBundle {
        source_path: "fixture/MyWork2".into(),
        statuses,
        service_points: vec!["Мира, 5".into(), "Ленина, 1".into()],
        dictionary,
        roles: vec![
            NewRole {
                name: "Администратор".into(),
                permissions: PermissionSet::all(),
            },
            NewRole {
                name: "Мастера".into(),
                permissions: PermissionSet::from_codes(["orders_edit"]),
            },
        ],
        users: vec![
            ImportedUser {
                login: "Ирина".into(),
                role: "Администратор".into(),
                legacy_sha1: mw_core::legacy_sha1_hex("1234"),
            },
            ImportedUser {
                login: "Олег".into(),
                role: "Мастера".into(),
                legacy_sha1: mw_core::legacy_sha1_hex("qwerty"),
            },
        ],
        settings: ImportedSettings {
            values: SettingsPatch {
                currency_symbol: Some("руб.".into()),
                firm_name: Some("СЕРВИС".into()),
                ..Default::default()
            },
            default_service_point: Some("Ленина, 1".into()),
        },
        clients: vec![
            client(
                7,
                "ИВАНОВ ИВАН",
                vec![
                    ClientPhone {
                        phone: "89211234567".into(),
                        note: String::new(),
                    },
                    ClientPhone {
                        phone: "89117654321".into(),
                        note: "жена".into(),
                    },
                ],
                dt(2014, 11, 2, 0, 0),
            ),
            client(9, "КИМ", Vec::new(), dt(2015, 3, 17, 12, 0)),
        ],
        orders: vec![o5, o6, o10],
        history: vec![
            ImportedHistory {
                at: dt(2015, 3, 5, 14, 20),
                user_name: String::new(),
                action: "legacy.status".into(),
                order_id: Some(5),
                client_id: None,
                summary: "Установлен статус «Диагностика»".into(),
                details: Some(json!({ "legacy": { "id": 1 } })),
            },
            ImportedHistory {
                at: dt(2015, 3, 7, 10, 0),
                user_name: "ADMIN".into(),
                action: "legacy.history".into(),
                order_id: Some(5),
                client_id: None,
                summary: "ПЕЧАТЬ АКТА ВЫДАЧИ".into(),
                details: None,
            },
        ],
        legacy_rows: vec![
            legacy_row("Catalog", "5", Some(LegacyTarget::Order(5))),
            legacy_row("ClientsMap", "7", Some(LegacyTarget::Client(7))),
            legacy_row("Users", "1", Some(LegacyTarget::User("Ирина".into()))),
            legacy_row(
                "GroupDostup",
                "1",
                Some(LegacyTarget::Role("Мастера".into())),
            ),
            legacy_row("Stock", "1", None),
        ],
        next_order_id: 16,
        next_client_id: 15,
        report: json!({ "warnings": [] }),
    }
}

fn legacy_row(
    source: &str,
    key: &str,
    target: Option<mw_core::LegacyTarget>,
) -> mw_core::ImportedLegacyRow {
    mw_core::ImportedLegacyRow {
        source: source.to_owned(),
        source_key: key.to_owned(),
        target,
        raw: serde_json::json!({ "id": key }),
    }
}

/// Цель следа импорта: (таблица, номер).
pub async fn legacy_target(core: &Core, source: &str, key: &str) -> Option<(String, i64)> {
    let row = mw_db::entity::legacy_row::Entity::find()
        .filter(mw_db::entity::legacy_row::Column::Source.eq(source))
        .filter(mw_db::entity::legacy_row::Column::SourceKey.eq(key))
        .one(core.db())
        .await
        .unwrap()
        .unwrap_or_else(|| panic!("нет следа {source}/{key}"));
    row.target_table.zip(row.target_id)
}

pub async fn service_point_id_by_name(core: &Core, name: &str) -> i64 {
    mw_db::entity::service_point::Entity::find()
        .filter(mw_db::entity::service_point::Column::Name.eq(name))
        .one(core.db())
        .await
        .unwrap()
        .unwrap_or_else(|| panic!("нет точки {name}"))
        .id
}

/// Заказ нового клиента с правкой полей перед созданием.
pub async fn create_order_with(t: &TestCore, tweak: impl FnOnce(&mut NewOrder)) -> mw_core::Order {
    let mut n = sample_new_order();
    tweak(&mut n);
    t.core.create_order(&admin(), n).await.unwrap()
}

/// Номера заказов (без удалённых) в порядке сортировки.
pub async fn list_sorted(
    t: &TestCore,
    field: mw_core::OrderSortField,
    descending: bool,
) -> Vec<i64> {
    ids(t
        .core
        .list_orders(
            &admin(),
            &OrderFilter::default(),
            OrderSort { field, descending },
            50,
            0,
        )
        .await
        .unwrap()
        .items)
}

pub fn sorted(mut v: Vec<i64>) -> Vec<i64> {
    v.sort_unstable();
    v
}

/// Записывает колонки ключей и `status_changed_at` в обход `Core`.
pub async fn set_order_keys_directly(
    t: &TestCore,
    id: i64,
    status_changed_at: Option<chrono::NaiveDateTime>,
    keys: &str,
) {
    order::ActiveModel {
        id: Set(id),
        status_changed_at: Set(status_changed_at),
        device_type_key: Set(keys.to_owned()),
        brand_key: Set(keys.to_owned()),
        model_key: Set(keys.to_owned()),
        master_key: Set(keys.to_owned()),
        ..Default::default()
    }
    .update(t.core.db())
    .await
    .unwrap();
}
