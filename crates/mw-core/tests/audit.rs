mod common;

use chrono::Duration;
use common::*;
use mw_core::{AuditFilter, CoreError, OrderPatch};
use mw_domain::permission::Permission;
use serde_json::json;

#[tokio::test]
async fn audit_listing_filters_permissions_and_order() -> Result<(), CoreError> {
    for t in cores().await {
        let o = t.core.create_order(&admin(), sample_new_order()).await?;
        t.clock.advance(Duration::minutes(5));
        let o = t
            .core
            .update_order(
                &admin(),
                o.id,
                1,
                OrderPatch {
                    need_call: Some(true),
                    ..Default::default()
                },
            )
            .await?;
        let anyone = actor_with(&[]);
        let hist = t
            .core
            .list_audit(
                &anyone,
                &AuditFilter {
                    order_id: Some(o.id),
                    ..Default::default()
                },
                50,
                0,
            )
            .await?;
        assert_eq!(
            hist.items
                .iter()
                .map(|e| e.action.as_str())
                .collect::<Vec<_>>(),
            ["order.updated", "order.created"],
            "{}",
            t.label
        );
        assert_eq!(hist.items[0].user_name, "Тест");
        assert_eq!(
            hist.items[0].details.as_ref().unwrap()["changes"]["need_call"],
            json!([false, true])
        );
        assert!(matches!(
            t.core
                .list_audit(&anyone, &AuditFilter::default(), 50, 0)
                .await,
            Err(CoreError::Forbidden(Permission::AuditView))
        ));
        let only_orders = t
            .core
            .list_audit(
                &admin(),
                &AuditFilter {
                    action_prefix: Some("order.".into()),
                    ..Default::default()
                },
                1,
                0,
            )
            .await?;
        assert_eq!((only_orders.items.len(), only_orders.total), (1, 2));
        let wildcard = t
            .core
            .list_audit(
                &admin(),
                &AuditFilter {
                    action_prefix: Some("order%".into()),
                    ..Default::default()
                },
                50,
                0,
            )
            .await?;
        assert_eq!((wildcard.items.len(), wildcard.total), (0, 0));
        let since = t
            .core
            .list_audit(
                &admin(),
                &AuditFilter {
                    from: Some(o.updated_at),
                    ..Default::default()
                },
                50,
                0,
            )
            .await?;
        assert_eq!(since.total, 1);
    }
    Ok(())
}
