mod common;

use chrono::Duration;
use common::*;
use mw_core::{CoreError, OrderPatch};
use mw_domain::permission::Permission;
use serde_json::json;

#[tokio::test]
async fn patch_bumps_version_audits_diff_and_masks_password() -> Result<(), CoreError> {
    for t in cores().await {
        let o = t.core.create_order(&admin(), sample_new_order()).await?;
        let u = t
            .core
            .update_order(
                &admin(),
                o.id,
                1,
                OrderPatch {
                    brand: Some("Apple".into()),
                    device_password: Some("1234".into()),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!((u.brand.as_str(), u.version), ("Apple", 2));
        let last = last_audit(&t.core, o.id).await;
        assert_eq!(last.action, "order.updated");
        assert_eq!(last.summary, "Изменено: бренд, пароль устройства");
        assert_eq!(
            last.details.clone().unwrap()["changes"]["brand"],
            json!(["Samsung", "Apple"])
        );
        assert_eq!(
            last.details.unwrap()["changes"]["device_password"],
            json!(["***", "***"])
        );
        assert!(
            !serde_json::to_string(&last_audit(&t.core, o.id).await.details)
                .unwrap()
                .contains("1234")
        );
        assert_eq!(
            t.core
                .update_order(&admin(), o.id, 2, OrderPatch::default())
                .await?
                .version,
            2
        );

        // stale_version_is_conflict_and_first_edit_survives
        let stale = t
            .core
            .update_order(
                &admin(),
                o.id,
                1,
                OrderPatch {
                    model: Some("A53".into()),
                    ..Default::default()
                },
            )
            .await;
        assert!(matches!(
            stale,
            Err(CoreError::Conflict {
                current_version: 2,
                ..
            })
        ));
        assert_eq!(t.core.get_order(&admin(), o.id).await?.model, "A52");

        // dates_need_extra_permission
        let editor = actor_with(&[Permission::OrdersEdit]);
        let patch = OrderPatch {
            received_at: Some(o.received_at - Duration::days(1)),
            ..Default::default()
        };
        assert!(matches!(
            t.core.update_order(&editor, o.id, 2, patch.clone()).await,
            Err(CoreError::Forbidden(Permission::OrderDatesEdit))
        ));
        assert!(t.core.update_order(&admin(), o.id, 2, patch).await.is_ok());
    }
    Ok(())
}

#[tokio::test]
async fn issue_requires_master_and_work_and_keeps_first_issue_date() -> Result<(), CoreError> {
    for t in cores().await {
        let o = t.core.create_order(&admin(), sample_new_order()).await?;
        assert_validation(
            t.core.issue_order(&admin(), o.id, 1).await,
            &[("master", "required"), ("work_done", "required")],
        );
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
        let first = t.core.issue_order(&admin(), o.id, o.version).await?;
        assert_eq!(first.issued_at.unwrap().to_string(), "2026-11-11 09:00:00");
        t.clock.advance(Duration::days(10));
        let warranty = status_id_by_name(&t.core, "Принят по гарантии").await;
        let back = t
            .core
            .change_status(&admin(), o.id, first.version, warranty)
            .await?;
        assert_eq!(back.issued_at, first.issued_at);
        t.clock.advance(Duration::days(2));
        let again = t.core.issue_order(&admin(), o.id, back.version).await?;
        assert_eq!(again.issued_at, first.issued_at);
        assert_eq!(
            last_audit(&t.core, o.id).await.summary,
            "Статус: «Принят по гарантии» → «Выдан»"
        );
        let same = t
            .core
            .change_status(&admin(), o.id, again.version, again.status_id)
            .await?;
        assert_eq!(same.version, again.version);
    }
    Ok(())
}

#[tokio::test]
async fn prepayment_and_clearing_rules() -> Result<(), CoreError> {
    use mw_domain::money::Money;
    for t in cores().await {
        let o = t.core.create_order(&admin(), sample_new_order()).await?;
        let u = t
            .core
            .update_order(
                &admin(),
                o.id,
                1,
                OrderPatch {
                    prepayment: Some(Money::from_rubles(500)),
                    final_price: Some(Some(Money::from_rubles(3000))),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(u.prepaid_at, Some(t.core.now()));
        let u = t
            .core
            .update_order(
                &admin(),
                o.id,
                2,
                OrderPatch {
                    prepayment: Some(Money::from_rubles(0)),
                    final_price: Some(None),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!((u.prepaid_at, u.final_price), (None, None));
        assert_validation(
            t.core
                .update_order(
                    &admin(),
                    o.id,
                    3,
                    OrderPatch {
                        estimate_from: Some(Some(Money::from_rubles(9))),
                        estimate_to: Some(Some(Money::from_rubles(1))),
                        client_id: Some(9999),
                        ..Default::default()
                    },
                )
                .await,
            &[("client_id", "invalid"), ("estimate_to", "estimate_range")],
        );
    }
    Ok(())
}

#[tokio::test]
async fn delete_is_soft_and_blocks_edits() -> Result<(), CoreError> {
    for t in cores().await {
        let o = t.core.create_order(&admin(), sample_new_order()).await?;
        assert!(matches!(
            t.core
                .delete_order(&actor_with(&[Permission::OrdersEdit]), o.id, 1)
                .await,
            Err(CoreError::Forbidden(Permission::OrdersDelete))
        ));
        t.core.delete_order(&admin(), o.id, 1).await?;
        assert!(t.core.get_order(&admin(), o.id).await?.deleted_at.is_some());
        assert_eq!(last_audit(&t.core, o.id).await.action, "order.deleted");
        assert_validation(
            t.core
                .update_order(
                    &admin(),
                    o.id,
                    2,
                    OrderPatch {
                        model: Some("X".into()),
                        ..Default::default()
                    },
                )
                .await,
            &[("id", "deleted")],
        );
        assert_validation(
            t.core.issue_order(&admin(), o.id, 2).await,
            &[("id", "deleted")],
        );
    }
    Ok(())
}
