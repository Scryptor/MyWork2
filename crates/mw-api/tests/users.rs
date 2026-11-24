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
        // Ни хэша, ни схемы пароля; только признак «пароль задан».
        let text = users.to_string();
        assert!(!text.contains("password_hash") && !text.contains("password_scheme"));
        assert_eq!(users[0]["has_password"], json!(true), "{}", t.label);
        let r = t
            .send(
                Method::POST,
                "/api/v1/roles",
                Some(&admin),
                Some(
                    json!({ "name": "Продавец", "permissions": ["orders_create", "future_perm"] }),
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
                Some(json!({
                    "login": "kassa", "display_name": "Касса", "role_id": role, "password": "1234",
                    "can_sign_in": true, "is_master": false
                })),
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
                Some(json!({
                    "login": "x", "display_name": "", "role_id": role, "password": "12",
                    "can_sign_in": true, "is_master": false
                })),
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

#[tokio::test]
async fn masters_without_sign_in_and_role_deletion_over_http() {
    for t in apis(Listen::Network).await {
        let admin = t.admin_token().await;
        let master_role = role_id_by_name(&t.core, "Мастер").await;
        let r = t
            .send(
                Method::POST,
                "/api/v1/users",
                Some(&admin),
                Some(json!({
                    "login": "", "display_name": "Пётр", "role_id": master_role, "password": "",
                    "can_sign_in": false, "is_master": true
                })),
            )
            .await;
        assert_eq!(r.status, 201, "{}: {}", t.label, r.body);
        assert_eq!(
            (
                &r.body["login"],
                &r.body["can_sign_in"],
                &r.body["is_master"]
            ),
            (&json!(""), &json!(false), &json!(true))
        );
        let petr = r.body["id"].as_i64().unwrap();

        // Список мастеров виден любому вошедшему, без права users_manage.
        let clerk = t.token_with("clerk", &[Permission::OrdersCreate]).await;
        let r = t.get("/api/v1/masters", Some(&clerk)).await;
        assert_eq!(
            (r.status, &r.body),
            (
                200,
                &json!([{ "id": petr, "display_name": "Пётр", "active": true }])
            ),
            "{}",
            t.label
        );
        assert_eq!(t.get("/api/v1/masters", None).await.status, 401);

        // Версия готовой роли — не 1: миграция склада дала ей просмотр склада.
        let roles = t.get("/api/v1/roles", Some(&admin)).await.body;
        let version = roles
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == json!(master_role))
            .unwrap()["version"]
            .clone();
        let r = t
            .send(
                Method::DELETE,
                &format!("/api/v1/roles/{master_role}?version={version}"),
                Some(&admin),
                None,
            )
            .await;
        assert_eq!(
            (r.status, &r.body["code"], &r.body["message"]),
            (
                409,
                &json!("conflict"),
                &json!("В роли есть пользователи: 1")
            ),
            "{}",
            t.label
        );
        let r = t
            .send(
                Method::POST,
                "/api/v1/roles",
                Some(&admin),
                Some(json!({ "name": "Склад", "permissions": ["stock_view"] })),
            )
            .await;
        let stock = r.body["id"].as_i64().unwrap();
        let path = format!("/api/v1/roles/{stock}?version=1");
        assert_eq!(
            t.send(Method::DELETE, &path, Some(&clerk), None)
                .await
                .status,
            403
        );
        assert_eq!(
            t.send(Method::DELETE, &path, Some(&admin), None)
                .await
                .status,
            204
        );
        let roles = t.get("/api/v1/roles", Some(&admin)).await.body;
        assert!(
            !roles
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["id"] == json!(stock))
        );
    }
}

#[tokio::test]
async fn masters_list_works_in_open_mode() {
    for t in apis(Listen::Loopback).await {
        let master_role = role_id_by_name(&t.core, "Мастер").await;
        let r = t
            .send(
                Method::POST,
                "/api/v1/users",
                None,
                Some(json!({
                    "login": "", "display_name": "Пётр", "role_id": master_role, "password": "",
                    "can_sign_in": false, "is_master": true
                })),
            )
            .await;
        assert_eq!(r.status, 201, "{}: {}", t.label, r.body);
        // Мастер без входа не отменяет режим без входа.
        let info = t.get("/api/v1/server-info", None).await.body;
        assert_eq!(
            (&info["auth_required"], &info["has_users"]),
            (&json!(false), &json!(false)),
            "{}",
            t.label
        );
        let r = t.get("/api/v1/masters", None).await;
        assert_eq!(
            (r.status, r.body.as_array().map(Vec::len)),
            (200, Some(1)),
            "{}",
            t.label
        );
    }
}

#[tokio::test]
async fn last_users_manage_role_is_not_deleted_over_http() {
    for t in apis(Listen::Loopback).await {
        let admin_role = role_id_by_name(&t.core, "Администратор").await;
        let r = t
            .send(
                Method::DELETE,
                &format!("/api/v1/roles/{admin_role}?version=1"),
                None,
                None,
            )
            .await;
        assert_eq!(
            (r.status, &r.body["code"], &r.body["message"]),
            (
                409,
                &json!("conflict"),
                &json!("Нельзя удалить последнюю роль с правом «Пользователи и права»")
            ),
            "{}",
            t.label
        );
    }
}

#[tokio::test]
async fn last_users_manage_role_keeps_the_permission_over_http() {
    for t in apis(Listen::Loopback).await {
        let admin_role = role_id_by_name(&t.core, "Администратор").await;
        let r = t
            .send(
                Method::PATCH,
                &format!("/api/v1/roles/{admin_role}?version=1"),
                None,
                Some(json!({ "permissions": ["orders_create"] })),
            )
            .await;
        assert_eq!(
            (r.status, &r.body["code"], &r.body["message"]),
            (
                409,
                &json!("conflict"),
                &json!("Нельзя снять право «Пользователи и права» с последней роли, где оно есть")
            ),
            "{}",
            t.label
        );
    }
}
