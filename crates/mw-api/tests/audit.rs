//! Журнал действий.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::Method;
use chrono::Duration;
use common::{Listen, apis, create_order, new_order};
use mw_domain::permission::Permission;
use serde_json::json;

#[tokio::test]
async fn audit_endpoints() {
    for t in apis(Listen::Network).await {
        let admin = t.admin_token().await;
        let clerk = t.token_with("kassa", &[Permission::OrdersCreate]).await;
        let id = create_order(&t, &clerk, new_order("Анна", "", "Samsung", "A52")).await["id"]
            .as_i64()
            .unwrap();
        t.clock.advance(Duration::minutes(5));
        assert_eq!(
            t.send(
                Method::PATCH,
                &format!("/api/v1/orders/{id}?version=1"),
                Some(&admin),
                Some(json!({ "need_call": true }))
            )
            .await
            .status,
            200
        );
        let hist = t
            .get(&format!("/api/v1/audit?order_id={id}"), Some(&clerk))
            .await
            .body;
        let actions: Vec<&str> = hist["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["action"].as_str().unwrap())
            .collect();
        assert_eq!(actions, ["order.updated", "order.created"], "{}", t.label);
        assert_eq!(
            hist["items"][0]["details"]["changes"]["need_call"],
            json!([false, true])
        );
        let r = t.get("/api/v1/audit", Some(&clerk)).await;
        assert_eq!(
            (r.status, &r.body["permission"]),
            (403, &json!("audit_view"))
        );
        let page = t
            .get("/api/v1/audit?action_prefix=order.&limit=1", Some(&admin))
            .await
            .body;
        assert_eq!(
            (page["items"].as_array().unwrap().len(), &page["total"]),
            (1, &json!(2))
        );
        assert_eq!(
            t.get(
                "/api/v1/audit?action_prefix=order.&from=2026-11-11T09:05:00",
                Some(&admin)
            )
            .await
            .body["total"],
            json!(1)
        );
        assert_eq!(
            t.get("/api/v1/audit?from=yesterday", Some(&admin))
                .await
                .status,
            400
        );
    }
}
