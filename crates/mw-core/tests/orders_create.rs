mod common;

use common::*;
use mw_core::{ClientRef, CoreError, NewClient, NewOrder, SettingsPatch};
use mw_domain::barcode::order_barcode;
use mw_domain::money::Money;
use mw_domain::permission::Permission;
use mw_domain::status::StatusKind;

#[tokio::test]
async fn intake_creates_client_order_barcode_and_audit() -> Result<(), CoreError> {
    for t in cores().await {
        let o = t
            .core
            .create_order(
                &actor_with(&[Permission::OrdersCreate]),
                NewOrder {
                    prepayment: Money::from_rubles(2000),
                    ..sample_new_order()
                },
            )
            .await?;
        assert_eq!(o.barcode, order_barcode(o.id).unwrap(), "{}", t.label);
        assert_eq!(status_name(&t.core, o.status_id).await, "Диагностика");
        assert_eq!(o.received_at.to_string(), "2026-11-11 09:00:00");
        assert_eq!(o.prepaid_at, Some(o.received_at));
        assert_eq!(t.core.get_order(&admin(), o.id).await?, o);
        assert_eq!(
            t.core.get_client(&admin(), o.client_id).await?.phone,
            "79211234567"
        );
        assert_eq!(
            audit_summaries(&t.core, o.id).await,
            vec![format!("Заказ № {} принят", o.id)]
        );
        let a = last_audit(&t.core, o.id).await;
        assert_eq!(a.action, "order.created");
        assert_eq!(count_audit(&t.core, "client.created").await, 0);
    }
    Ok(())
}

#[tokio::test]
async fn required_fields_follow_setting() -> Result<(), CoreError> {
    for t in cores().await {
        let empty = NewOrder {
            client: ClientRef::New(NewClient {
                full_name: "Ким".into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_validation(
            t.core.create_order(&admin(), empty.clone()).await,
            &[
                ("device_type", "required"),
                ("brand", "required"),
                ("model", "required"),
                ("malfunction", "required"),
            ],
        );
        t.core
            .update_settings(
                &admin(),
                SettingsPatch {
                    intake_required_fields: Some(false),
                    ..Default::default()
                },
            )
            .await?;
        assert!(t.core.create_order(&admin(), empty).await.is_ok());
    }
    Ok(())
}

#[tokio::test]
async fn invalid_references_and_estimate_range() -> Result<(), CoreError> {
    for t in cores().await {
        let issued_id = status_id_by_kind(&t.core, StatusKind::Issued).await;
        let bad = NewOrder {
            client: ClientRef::Existing(9_999),
            service_point_id: Some(9_999),
            status_id: Some(issued_id),
            estimate_from: Some(Money::from_rubles(7000)),
            estimate_to: Some(Money::from_rubles(6000)),
            ..sample_new_order()
        };
        assert_validation(
            t.core.create_order(&admin(), bad).await,
            &[
                ("estimate_to", "estimate_range"),
                ("client_id", "invalid"),
                ("service_point_id", "invalid"),
                ("status_id", "invalid"),
            ],
        );
        assert!(matches!(
            t.core.get_order(&admin(), 9_999).await,
            Err(CoreError::NotFound { .. })
        ));
    }
    Ok(())
}

#[tokio::test]
async fn new_client_needs_name_or_phone_and_rights_are_checked() -> Result<(), CoreError> {
    for t in cores().await {
        let no_client = NewOrder {
            client: ClientRef::default(),
            ..sample_new_order()
        };
        assert_validation(
            t.core.create_order(&admin(), no_client).await,
            &[("full_name", "required")],
        );
        assert!(matches!(
            t.core
                .create_order(
                    &actor_with(&[Permission::ClientsCreate]),
                    sample_new_order()
                )
                .await,
            Err(CoreError::Forbidden(_))
        ));
        let o = t.core.create_order(&admin(), sample_new_order()).await?;
        let again = t
            .core
            .create_order(
                &admin(),
                NewOrder {
                    client: ClientRef::Existing(o.client_id),
                    ..sample_new_order()
                },
            )
            .await?;
        assert_eq!(again.client_id, o.client_id);
        assert_ne!(again.id, o.id);
    }
    Ok(())
}
