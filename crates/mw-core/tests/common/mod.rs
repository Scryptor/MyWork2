//! Общие помощники тестов `mw-core`.
//!
//! A `// name` block in the plan's test listings that uses variables from an earlier block continues that block's test function; all other blocks are separate test functions with fresh `cores()`.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::fmt::Debug;
use std::sync::Arc;

use chrono::{TimeZone, Utc};
use mw_core::{
    Actor, ClientRef, Core, CoreError, FixedClock, NewClient, NewOrder, OrderFilter, OrderPatch,
    OrderSort,
};
use mw_db::entity::{audit_log, order, role, session, status, user};
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
    let mut actor = Actor::unrestricted("Тест");
    actor.permissions = mw_domain::permission::PermissionSet::empty();
    for p in permissions {
        actor.permissions.insert(*p);
    }
    actor
}

/// Ровно эти пары (поле, код) в этом порядке.
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
            phone: "+7 921 123-45-67".into(),
            ..Default::default()
        }),
        device_type: "Телефон".into(),
        brand: "Samsung".into(),
        model: "A52".into(),
        malfunction: "Разбит экран".into(),
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
            phone: phone.into(),
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
