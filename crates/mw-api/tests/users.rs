//! Пользователи и роли.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::Method;
use common::{Listen, apis, role_id_by_name};
use mw_domain::permission::Permission;
use serde_json::json;

#[tokio::test]
async fn users_and_roles() {
    for t in apis(Listen::Network).await {
        let admin = t.admin_token().await;
        let users = t.get("/api/v1/users", Some(&admin)).await.body;
        assert_eq!(users.as_array().unwrap().len(), 1, "{}", t.label);
        assert!(!users.to_string().contains("password"));
        let r = t
            .send(
                Method::POST,
                "/api/v1/roles",
                Some(&admin),
                Some(
                    json!({ "name": "Приёмщик", "permissions": ["orders_create", "future_perm"] }),
                ),
            )
            .await;
        assert_eq!(
            (r.status, &r.body["permissions"]),
            (201, &json!(["orders_create"]))
        );
        let role = r.body["id"].as_i64().unwrap();
        let r = t
            .send(
                Method::POST,
                "/api/v1/users",
                Some(&admin),
                Some(json!({ "login": "kassa", "display_name": "Касса", "role_id": role, "password": "1234" })),
            )
            .await;
        assert_eq!(r.status, 201);
        assert!(r.body.get("password").is_none());
        let kassa = r.body["id"].as_i64().unwrap();
        let r = t
            .send(
                Method::POST,
                "/api/v1/users",
                Some(&admin),
                Some(
                    json!({ "login": "x", "display_name": "", "role_id": role, "password": "12" }),
                ),
            )
            .await;
        assert_eq!(
            (r.status, &r.body["fields"][0]["field"]),
            (422, &json!("password"))
        );
        let r = t
            .send(
                Method::PATCH,
                &format!("/api/v1/users/{kassa}?version=1"),
                Some(&admin),
                Some(json!({ "active": false })),
            )
            .await;
        assert_eq!(
            (&r.body["active"], &r.body["version"]),
            (&json!(false), &json!(2))
        );
        assert_eq!(
            t.send(
                Method::PUT,
                &format!("/api/v1/users/{kassa}/password"),
                Some(&admin),
                Some(json!({ "password": "zzzz" }))
            )
            .await
            .status,
            204
        );
        let r = t
            .send(
                Method::PATCH,
                &format!("/api/v1/roles/{role}?version=1"),
                Some(&admin),
                Some(json!({ "name": "Касса" })),
            )
            .await;
        assert_eq!(
            (&r.body["name"], &r.body["version"]),
            (&json!("Касса"), &json!(2))
        );
        let clerk = t.token_with("clerk", &[Permission::OrdersCreate]).await;
        let r = t.get("/api/v1/users", Some(&clerk)).await;
        assert_eq!(
            (r.status, &r.body["permission"]),
            (403, &json!("users_manage"))
        );
    }
}

#[tokio::test]
async fn last_admin_is_protected_over_http() {
    for t in apis(Listen::Network).await {
        let admin = t.admin_token().await;
        let me = t.get("/api/v1/auth/me", Some(&admin)).await.body["user_id"]
            .as_i64()
            .unwrap();
        let r = t
            .send(
                Method::PATCH,
                &format!("/api/v1/users/{me}?version=1"),
                Some(&admin),
                Some(json!({ "active": false })),
            )
            .await;
        assert_eq!(
            (r.status, &r.body["fields"][0]),
            (
                422,
                &json!({ "field": "active", "code": "last_admin", "message": "Нельзя оставить программу без администратора" })
            )
        );
        let admin_role = role_id_by_name(&t.core, "Администратор").await;
        let r = t
            .send(
                Method::PATCH,
                &format!("/api/v1/roles/{admin_role}?version=1"),
                Some(&admin),
                Some(json!({ "permissions": ["orders_create"] })),
            )
            .await;
        assert_eq!(r.body["fields"][0]["code"], json!("last_admin"));
    }
}
