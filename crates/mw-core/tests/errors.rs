mod common;

use chrono::{Duration, TimeZone, Utc};
use common::*;
use mw_core::{Actor, CoreError};
use mw_domain::permission::{Permission, PermissionSet};

#[tokio::test]
async fn corrupted_codes_in_db_are_internal_errors() -> Result<(), CoreError> {
    for t in cores().await {
        let ready = status_id_by_name(&t.core, "Готов").await;
        set_status_kind_directly(&t, ready, "bogus").await;
        match t.core.list_statuses(true).await {
            Err(CoreError::Internal(msg)) => {
                assert!(
                    msg.contains(&format!("статус №{ready}")),
                    "{}: {msg}",
                    t.label
                )
            }
            other => panic!("{}: {other:?}", t.label),
        }
    }
    assert_eq!(
        CoreError::Internal("x".into()).to_string(),
        "внутренняя ошибка: x"
    );
    Ok(())
}

#[tokio::test]
async fn now_utc_reads_core_clock() -> Result<(), CoreError> {
    for t in cores().await {
        assert_eq!(
            t.core.now_utc(),
            Utc.with_ymd_and_hms(2026, 11, 11, 9, 0, 0).unwrap()
        );
        t.clock.advance(Duration::minutes(5));
        assert_eq!(t.core.now_utc().to_rfc3339(), "2026-11-11T09:05:00+00:00");
    }
    Ok(())
}

#[test]
fn actor_is_read_only_outside_core() {
    let a = Actor::for_tests(
        Some(7),
        "Ирина",
        PermissionSet::from_codes(["orders_create"]),
    );
    assert_eq!((a.user_id(), a.name()), (Some(7), "Ирина"));
    assert!(
        a.permissions().contains(Permission::OrdersCreate)
            && a.require(Permission::UsersManage).is_err()
    );
    assert_eq!(
        Actor::unrestricted("Сервер").permissions(),
        &PermissionSet::all()
    );
}
