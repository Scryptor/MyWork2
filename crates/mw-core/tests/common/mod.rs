//! Общие помощники тестов `mw-core`.
//!
//! A `// name` block in the plan's test listings that uses variables from an earlier block continues that block's test function; all other blocks are separate test functions with fresh `cores()`.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::fmt::Debug;
use std::sync::Arc;

use chrono::{TimeZone, Utc};
use mw_core::{Actor, Core, CoreError, FixedClock};
use mw_db::entity::{audit_log, role, status};
use mw_domain::permission::Permission;
use mw_domain::status::StatusKind;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder};

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
