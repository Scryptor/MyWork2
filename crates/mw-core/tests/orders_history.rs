mod common;

use chrono::{Duration, NaiveDateTime};
use common::*;
use mw_core::{CoreError, NewWarrantyCase, OrderPatch, PageRequest};
use mw_domain::permission::Permission;
use mw_domain::status::StatusKind;
use sea_orm::{ActiveModelTrait, Set};

async fn issued_order(t: &TestCore) -> Result<mw_core::Order, CoreError> {
    let o = t.core.create_order(&admin(), sample_new_order()).await?;
    let o = t
        .core
        .update_order(
            &admin(),
            o.id,
            1,
            OrderPatch {
                master: Some("Антон".into()),
                work_done: Some("Замена дисплея".into()),
                ..Default::default()
            },
        )
        .await?;
    t.core.issue_order(&admin(), o.id, o.version).await
}

fn case(complaint: &str) -> NewWarrantyCase {
    NewWarrantyCase {
        complaint: complaint.to_owned(),
        at: None,
    }
}

#[tokio::test]
async fn order_history_needs_no_audit_permission() -> Result<(), CoreError> {
    for t in cores().await {
        let o = t.core.create_order(&admin(), sample_new_order()).await?;
        t.clock.advance(Duration::minutes(5));
        let ready = status_id_by_kind(&t.core, StatusKind::Ready).await;
        t.core.change_status(&admin(), o.id, 1, ready).await?;
        let editor = actor_with(&[Permission::OrdersEdit]);
        let page = t
            .core
            .order_history(&editor, o.id, PageRequest::default())
            .await?;
        assert_eq!(page.total, 2, "{}", t.label);
        assert_eq!(page.items[0].action, "order.status_changed");
        assert_eq!(page.items[1].action, "order.created");
        let first = t
            .core
            .order_history(
                &editor,
                o.id,
                PageRequest {
                    limit: 1,
                    offset: 1,
                },
            )
            .await?;
        assert_eq!(first.items.len(), 1);
        assert_eq!(first.items[0].action, "order.created");
        assert!(matches!(
            t.core.list_audit(&editor, &Default::default(), 50, 0).await,
            Err(CoreError::Forbidden(Permission::AuditView))
        ));
        assert!(matches!(
            t.core
                .order_history(&editor, 9999, PageRequest::default())
                .await,
            Err(CoreError::NotFound { .. })
        ));
        assert_validation(
            t.core
                .order_history(
                    &editor,
                    o.id,
                    PageRequest {
                        limit: 0,
                        offset: 0,
                    },
                )
                .await,
            &[("limit", "out_of_range")],
        );
    }
    Ok(())
}

#[tokio::test]
async fn warranty_case_moves_an_issued_order_and_writes_history() -> Result<(), CoreError> {
    for t in cores().await {
        let issued = issued_order(&t).await?;
        let issued_at = issued.issued_at;
        assert!(issued_at.is_some());
        t.clock.advance(Duration::days(3));
        let w = t
            .core
            .accept_warranty_case(
                &admin(),
                issued.id,
                issued.version,
                case("Снова не заряжается"),
            )
            .await?;
        assert_eq!(
            w.status_id,
            status_id_by_name(&t.core, "Принят по гарантии").await,
            "{}",
            t.label
        );
        assert_eq!(w.issued_at, issued_at);
        assert_eq!(
            status_changed_at(&t.core, w.id).await,
            Some("2026-11-14 09:00:00".to_owned())
        );
        let last = last_audit(&t.core, issued.id).await;
        assert_eq!(last.action, "order.warranty_case");
        assert_eq!(last.summary, "Принят по гарантии: Снова не заряжается");
        assert_eq!(
            last.details,
            Some(serde_json::json!({
                "complaint": "Снова не заряжается",
                "at": "2026-11-14T09:00:00"
            }))
        );
        let summaries = audit_summaries(&t.core, issued.id).await;
        assert!(
            summaries
                .iter()
                .any(|s| s == "Статус: «Выдан» → «Принят по гарантии»")
        );
    }
    Ok(())
}

#[tokio::test]
async fn warranty_case_rejects_wrong_input() -> Result<(), CoreError> {
    for t in cores().await {
        let o = t.core.create_order(&admin(), sample_new_order()).await?;
        assert_validation(
            t.core
                .accept_warranty_case(&admin(), o.id, o.version, case("Сломалось"))
                .await,
            &[("status", "invalid")],
        );
        let issued = issued_order(&t).await?;
        assert_validation(
            t.core
                .accept_warranty_case(&admin(), issued.id, issued.version, case("  "))
                .await,
            &[("complaint", "required")],
        );
        assert!(matches!(
            t.core
                .accept_warranty_case(&actor_with(&[]), issued.id, issued.version, case("x"))
                .await,
            Err(CoreError::Forbidden(Permission::OrdersEdit))
        ));
        assert!(matches!(
            t.core
                .accept_warranty_case(&admin(), issued.id, issued.version + 5, case("x"))
                .await,
            Err(CoreError::Conflict { .. })
        ));
        let warranty = status_id_by_name(&t.core, "Принят по гарантии").await;
        mw_db::entity::status::ActiveModel {
            id: Set(warranty),
            archived: Set(true),
            ..Default::default()
        }
        .update(t.core.db())
        .await
        .unwrap();
        assert_validation(
            t.core
                .accept_warranty_case(&admin(), issued.id, issued.version, case("Сломалось"))
                .await,
            &[("status", "invalid")],
        );
    }
    Ok(())
}

/// Жалоба с датой обращения; строки дат заданы в тесте руками.
#[allow(clippy::unwrap_used)]
fn case_on(at: &str) -> NewWarrantyCase {
    NewWarrantyCase {
        complaint: "Сломалось".to_owned(),
        at: Some(NaiveDateTime::parse_from_str(at, "%Y-%m-%dT%H:%M:%S").unwrap()),
    }
}

#[tokio::test]
async fn warranty_case_day_lies_between_the_issue_and_now() -> Result<(), CoreError> {
    for t in cores().await {
        // Выданы 2026-11-11 в 09:00, обращение — через три дня.
        let first = issued_order(&t).await?;
        let second = issued_order(&t).await?;
        let undated = issued_order(&t).await?;
        let undated = t
            .core
            .update_order(
                &admin(),
                undated.id,
                undated.version,
                OrderPatch {
                    issued_at: Some(None),
                    ..Default::default()
                },
            )
            .await?;
        t.clock.advance(Duration::days(3));
        for at in [
            "2026-11-14T09:00:01",
            "2026-11-15T00:00:00",
            "2026-11-10T23:59:59",
        ] {
            assert_validation(
                t.core
                    .accept_warranty_case(&admin(), first.id, first.version, case_on(at))
                    .await,
                &[("at", "invalid")],
            );
        }
        // Окно выбирает день: день выдачи годится и с полуночи.
        t.core
            .accept_warranty_case(
                &admin(),
                first.id,
                first.version,
                case_on("2026-11-11T00:00:00"),
            )
            .await?;
        t.core
            .accept_warranty_case(
                &admin(),
                second.id,
                second.version,
                case_on("2026-11-14T09:00:00"),
            )
            .await?;
        assert_eq!(
            last_audit(&t.core, second.id).await.details,
            Some(serde_json::json!({ "complaint": "Сломалось", "at": "2026-11-14T09:00:00" })),
            "{}",
            t.label
        );
        // Без даты выдачи проверяется только «не позже сейчас».
        t.core
            .accept_warranty_case(
                &admin(),
                undated.id,
                undated.version,
                case_on("2026-01-05T00:00:00"),
            )
            .await?;
    }
    Ok(())
}
