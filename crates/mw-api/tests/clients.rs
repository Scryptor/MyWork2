//! Клиенты: список, поиск, создание, правка с версией.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::Method;
use common::{Listen, apis};
use mw_domain::permission::Permission;
use serde_json::json;

#[tokio::test]
async fn client_endpoints() {
    for t in apis(Listen::Network).await {
        let token = t
            .token_with(
                "kassa",
                &[
                    Permission::ClientsView,
                    Permission::ClientsCreate,
                    Permission::ClientsEdit,
                ],
            )
            .await;
        let new = json!({ "full_name": " ПЁТР ИВАНОВ ", "phones": [{"phone": "+7 (921) 123-45-67", "note": ""}, {"phone": "8 911 765-43-21", "note": "жена"}], "address": "", "note": "", "source": "", "blacklisted": false });
        let r = t
            .send(Method::POST, "/api/v1/clients", Some(&token), Some(new))
            .await;
        assert_eq!(
            (r.status, &r.body["phones"][1], &r.body["version"]),
            (
                201,
                &json!({"phone": "89117654321", "note": "жена"}),
                &json!(1)
            ),
            "{}",
            t.label
        );
        let id = r.body["id"].as_i64().unwrap();
        let page = t
            .get(
                "/api/v1/clients?query=%D0%BF%D1%91%D1%82%D1%80",
                Some(&token),
            )
            .await
            .body;
        assert_eq!(
            (&page["total"], &page["items"][0]["id"]),
            (&json!(1), &json!(id)),
            "{}",
            t.label
        );
        assert_eq!(
            t.get("/api/v1/clients/by-phone?phone=89117654321", Some(&token))
                .await
                .body[0]["id"],
            json!(id),
            "{}",
            t.label
        );
        let old_shape = json!({ "full_name": "А", "phone": "1", "address": "", "note": "", "source": "", "blacklisted": false });
        assert_eq!(
            t.send(
                Method::POST,
                "/api/v1/clients",
                Some(&token),
                Some(old_shape)
            )
            .await
            .status,
            400,
            "{}",
            t.label
        );
        assert_eq!(
            t.get(
                "/api/v1/clients/by-phone?phone=8%20921%20123%2045%2067",
                Some(&token)
            )
            .await
            .body[0]["id"],
            json!(id),
            "{}",
            t.label
        );
        assert_eq!(
            t.get(&format!("/api/v1/clients/{id}"), Some(&token))
                .await
                .body["full_name"],
            json!("ПЁТР ИВАНОВ"),
            "{}",
            t.label
        );
        let r = t.get("/api/v1/clients?limit=0", Some(&token)).await;
        assert_eq!(
            (r.status, &r.body["fields"][0]["field"]),
            (422, &json!("limit")),
            "{}",
            t.label
        );
        assert_eq!(
            t.get("/api/v1/clients?page=2", Some(&token)).await.status,
            400,
            "{}",
            t.label
        );
        let other = t.token_with("master", &[Permission::OrdersEdit]).await;
        let r = t.get("/api/v1/clients", Some(&other)).await;
        assert_eq!(
            (r.status, &r.body["permission"]),
            (403, &json!("clients_view")),
            "{}",
            t.label
        );
    }
}

#[tokio::test]
async fn client_versions_over_http() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        let new = json!({ "full_name": "Анна", "phones": [], "address": "", "note": "", "source": "", "blacklisted": false });
        let r = t
            .send(Method::POST, "/api/v1/clients", Some(&token), Some(new))
            .await;
        assert_eq!(r.status, 201, "{}", t.label);
        let id = r.body["id"].as_i64().unwrap();
        let patch = async |v: i64, body: serde_json::Value| {
            t.send(
                Method::PATCH,
                &format!("/api/v1/clients/{id}?version={v}"),
                Some(&token),
                Some(body),
            )
            .await
        };
        assert_eq!(
            patch(1, json!({ "address": "Ленина, 1" })).await.body["version"],
            json!(2),
            "{}",
            t.label
        );
        let r = patch(1, json!({ "address": "Мира, 2" })).await;
        assert_eq!(
            (r.status, &r.body["code"], &r.body["current_version"]),
            (409, &json!("conflict"), &json!(2)),
            "{}",
            t.label
        );
        let r = patch(1, json!({ "address": " Ленина, 1 " })).await;
        assert_eq!(
            (r.status, &r.body["version"]),
            (200, &json!(2)),
            "{}",
            t.label
        );
        assert_eq!(
            patch(2, json!({ "blacklisted": null })).await.status,
            400,
            "{}",
            t.label
        );
        let r = t
            .send(
                Method::PATCH,
                &format!("/api/v1/clients/{id}"),
                Some(&token),
                Some(json!({ "note": "x" })),
            )
            .await;
        assert_eq!(
            (r.status, &r.body["code"]),
            (400, &json!("bad_request")),
            "{}",
            t.label
        );
        assert_eq!(
            t.get(&format!("/api/v1/clients/{id}"), Some(&token))
                .await
                .body["address"],
            json!("Ленина, 1"),
            "{}",
            t.label
        );
    }
}
