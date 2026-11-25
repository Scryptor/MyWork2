//! Платежи по заказу в API: приём, список, удаление, ошибки и права.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::Method;
use common::{Listen, apis, create_order, new_order};
use mw_domain::permission::Permission;
use serde_json::json;

#[tokio::test]
async fn payments_are_added_listed_and_removed() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        let o = create_order(&t, &token, new_order("Анна", "", "Samsung", "A52")).await;
        let id = o["id"].as_i64().unwrap();
        let url = format!("/api/v1/orders/{id}/payments");
        let r = t
            .send(
                Method::POST,
                &url,
                Some(&token),
                Some(json!({
                    "day": "2026-11-11", "amount": 150_000,
                    "received_by": "Иванов", "request_key": "k-1"
                })),
            )
            .await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        assert_eq!(r.body["payments"][0]["day"], json!("2026-11-11"));
        assert_eq!(r.body["paid"], json!(150_000));
        assert_eq!(t.get(&url, Some(&token)).await.body, r.body);
        let pid = r.body["payments"][0]["id"].as_i64().unwrap();
        let del = format!("{url}/{pid}");
        let r = t.send(Method::DELETE, &del, Some(&token), None).await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        assert_eq!(r.body["paid"], json!(0));
        let r = t.send(Method::DELETE, &del, Some(&token), None).await;
        assert_eq!(r.status, 404);
    }
}

#[tokio::test]
async fn wrong_payments_answer_422_or_400() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        let o = create_order(&t, &token, new_order("Анна", "", "Samsung", "A52")).await;
        let url = format!("/api/v1/orders/{}/payments", o["id"]);
        let post = async |body: serde_json::Value| {
            t.send(Method::POST, &url, Some(&token), Some(body)).await
        };
        let r = post(json!({
            "day": "2026-11-12", "amount": 100, "received_by": "", "request_key": "a"
        }))
        .await;
        assert_eq!(r.status, 422, "{}: {}", t.label, r.body);
        assert_eq!(r.body["fields"][0]["field"], json!("day"));
        assert_eq!(r.body["fields"][0]["code"], json!("payment_day"));
        assert!(!r.body["fields"][0]["message"].as_str().unwrap().is_empty());
        let r = post(json!({
            "day": "2026-11-11", "amount": 0, "received_by": "", "request_key": "b"
        }))
        .await;
        assert_eq!(r.status, 422);
        assert_eq!(r.body["fields"][0]["field"], json!("amount"));
        assert_eq!(r.body["fields"][0]["code"], json!("out_of_range"));
        let r = post(json!({
            "day": "2026-11-11", "amount": 100, "received_by": "", "request_key": "c",
            "extra": 1
        }))
        .await;
        assert_eq!(r.status, 400);
        let r = post(json!({ "day": "2026-11-11", "amount": 100, "received_by": "" })).await;
        assert_eq!(r.status, 400);
    }
}

#[tokio::test]
async fn payment_rights() {
    for t in apis(Listen::Network).await {
        let admin = t.admin_token().await;
        let weak = t.token_with("kassa", &[Permission::OrdersCreate]).await;
        let o = create_order(&t, &admin, new_order("Анна", "", "Samsung", "A52")).await;
        let url = format!("/api/v1/orders/{}/payments", o["id"]);
        let r = t
            .send(
                Method::POST,
                &url,
                Some(&weak),
                Some(json!({
                    "day": "2026-11-11", "amount": 100, "received_by": "", "request_key": "a"
                })),
            )
            .await;
        assert_eq!(r.status, 403, "{}", t.label);
        let r = t
            .send(Method::DELETE, &format!("{url}/1"), Some(&weak), None)
            .await;
        assert_eq!(r.status, 403);
        assert_eq!(t.get(&url, Some(&weak)).await.status, 200);
    }
}
