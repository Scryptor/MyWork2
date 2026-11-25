//! Карта склада по HTTP: шкафы.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::Method;
use common::{Listen, apis};
use mw_domain::permission::Permission;
use serde_json::json;

fn body(code: &str) -> serde_json::Value {
    json!({ "code": code, "label": "Дисплеи", "shelves": 3, "cells_per_shelf": 5,
        "service_point_id": null })
}

#[tokio::test]
async fn racks_round_trip() {
    for t in apis(Listen::Network).await {
        let token = t
            .token_with("sklad", &[Permission::StockView, Permission::StockEdit])
            .await;
        let send = async |m: Method, path: &str, b: Option<serde_json::Value>| {
            t.send(m, path, Some(&token), b).await
        };
        let r = send(Method::POST, "/api/v1/stock/racks", Some(body("b"))).await;
        assert_eq!(
            (r.status, &r.body["code"]),
            (201, &json!("B")),
            "{}: {}",
            t.label,
            r.body
        );
        let b = r.body["id"].as_i64().unwrap();
        let r = send(Method::POST, "/api/v1/stock/racks", Some(body("a"))).await;
        let a = r.body["id"].as_i64().unwrap();

        let r = send(Method::GET, "/api/v1/stock/racks", None).await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        assert_eq!(r.body.as_array().unwrap().len(), 2);
        assert_eq!(r.body[0]["cells"].as_array().unwrap().len(), 15);
        assert_eq!(r.body[0]["cells"][0]["code"], json!("B-1-1"));
        let r = send(
            Method::GET,
            "/api/v1/stock/racks?query=x&restock=true",
            None,
        )
        .await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);

        let r = send(
            Method::PATCH,
            &format!("/api/v1/stock/racks/{b}"),
            Some(json!({ "label": "Новая" })),
        )
        .await;
        assert_eq!(
            (r.status, &r.body["label"]),
            (200, &json!("Новая")),
            "{}",
            t.label
        );

        let r = send(
            Method::PUT,
            "/api/v1/stock/racks/order",
            Some(json!({ "ordered_ids": [a, b], "seen_ids": [b, a] })),
        )
        .await;
        assert_eq!(r.status, 204, "{}: {}", t.label, r.body);
        let r = send(Method::GET, "/api/v1/stock/racks", None).await;
        assert_eq!(r.body[0]["code"], json!("A"));

        let r = send(Method::DELETE, &format!("/api/v1/stock/racks/{b}"), None).await;
        assert_eq!(r.status, 204, "{}: {}", t.label, r.body);
    }
}

#[tokio::test]
async fn rack_errors() {
    for t in apis(Listen::Network).await {
        let token = t
            .token_with("sklad", &[Permission::StockView, Permission::StockEdit])
            .await;
        let viewer = t.token_with("smotrit", &[Permission::StockView]).await;
        let r = t
            .send(
                Method::POST,
                "/api/v1/stock/racks",
                Some(&token),
                Some(body("B")),
            )
            .await;
        assert_eq!(r.status, 201, "{}: {}", t.label, r.body);

        let r = t
            .send(
                Method::POST,
                "/api/v1/stock/racks",
                Some(&token),
                Some(body("b")),
            )
            .await;
        assert_eq!(r.status, 422, "{}: {}", t.label, r.body);
        assert!(r.body.to_string().contains("duplicate"), "{}", r.body);

        // Лишнее поле — 400, как у остальных тел запросов.
        let mut extra = body("C");
        extra["extra"] = json!(1);
        let r = t
            .send(
                Method::POST,
                "/api/v1/stock/racks",
                Some(&token),
                Some(extra),
            )
            .await;
        assert_eq!(r.status, 400, "{}: {}", t.label, r.body);

        let r = t
            .send(
                Method::DELETE,
                "/api/v1/stock/racks/9999",
                Some(&token),
                None,
            )
            .await;
        assert_eq!(r.status, 404, "{}: {}", t.label, r.body);

        let r = t
            .send(
                Method::POST,
                "/api/v1/stock/racks",
                Some(&viewer),
                Some(body("D")),
            )
            .await;
        assert_eq!(r.status, 403, "{}: {}", t.label, r.body);
    }
}
