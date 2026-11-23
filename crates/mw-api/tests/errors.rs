//! Единые ошибки, защита запроса, `server-info` и `openapi.json`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::body::Body;
use axum::http::{Method, Request};
use common::{Listen, apis, get_with_host, role_id_by_name};
use mw_core::{Actor, NewUser};
use serde_json::json;

#[tokio::test]
async fn unknown_paths_and_methods_answer_with_json_errors() {
    for t in apis(Listen::Network).await {
        let r = t.get("/api/v1/nope", None).await;
        assert_eq!(
            (r.status, &r.body["code"]),
            (404, &json!("not_found")),
            "{}",
            t.label
        );
        // Ошибки своего кода есть в каждом ответе, пустые у всех, кроме ошибки кода.
        assert_eq!(r.body["diagnostics"], json!([]), "{}", t.label);
        let r = t
            .send(Method::DELETE, "/api/v1/server-info", None, None)
            .await;
        assert_eq!(
            (r.status, &r.body["code"], &r.body["fields"]),
            (405, &json!("bad_request"), &json!([])),
            "{}",
            t.label
        );
    }
}

#[tokio::test]
async fn server_info_tells_whether_login_is_needed() {
    for t in apis(Listen::Loopback).await {
        let r = t.get("/api/v1/server-info", None).await;
        assert_eq!(
            r.body,
            json!({ "app": "mw", "version": env!("CARGO_PKG_VERSION"), "api_version": 1,
                    "auth_required": false, "has_users": false, "setup_required": true,
                    "server_time": "2026-11-11T09:00:00" }),
            "{}",
            t.label
        );
        let role = role_id_by_name(&t.core, "Администратор").await;
        t.core
            .create_user(
                &Actor::unrestricted("Тест"),
                NewUser {
                    login: "boss".into(),
                    display_name: String::new(),
                    role_id: role,
                    password: "1234".into(),
                    can_sign_in: true,
                    is_master: false,
                },
            )
            .await
            .unwrap();
        let r = t.get("/api/v1/server-info", None).await;
        assert_eq!(
            (&r.body["auth_required"], &r.body["has_users"]),
            (&json!(true), &json!(true)),
            "{}",
            t.label
        );
    }
    for t in apis(Listen::Network).await {
        assert_eq!(
            t.get("/api/v1/server-info", None).await.body["auth_required"],
            json!(true),
            "{}",
            t.label
        );
    }
}

#[tokio::test]
async fn changing_methods_need_json_content_type() {
    for t in apis(Listen::Network).await {
        let no_json = Request::post("/api/v1/server-info")
            .header("host", "127.0.0.1")
            .body(Body::empty())
            .unwrap();
        let r = t.request(no_json).await;
        assert_eq!(
            (r.status, &r.body["code"]),
            (415, &json!("bad_request")),
            "{}",
            t.label
        );
        assert_eq!(r.headers["cache-control"], "no-store", "{}", t.label);
        assert_eq!(r.headers["x-content-type-options"], "nosniff");
        // с JSON доходит до роутера
        assert_eq!(
            t.send(Method::POST, "/api/v1/server-info", None, None)
                .await
                .status,
            405,
            "{}",
            t.label
        );
    }
}

#[tokio::test]
async fn loopback_server_accepts_only_local_host_names() {
    for t in apis(Listen::Loopback).await {
        for host in ["127.0.0.1:8080", "localhost", "LOCALHOST:1", "[::1]:8080"] {
            assert_eq!(
                t.request(get_with_host("/api/v1/server-info", host))
                    .await
                    .status,
                200,
                "{} {host}",
                t.label
            );
        }
        for host in [
            "evil.example",
            "127.0.0.1.evil.example:8080",
            "192.168.1.5:8080",
        ] {
            let r = t.request(get_with_host("/api/v1/server-info", host)).await;
            assert_eq!(
                (r.status, &r.body["code"]),
                (403, &json!("forbidden")),
                "{} {host}",
                t.label
            );
            assert_eq!(r.headers["cache-control"], "no-store", "{host}");
            assert_eq!(r.headers["x-content-type-options"], "nosniff", "{host}");
        }
    }
    for t in apis(Listen::Network).await {
        assert_eq!(
            t.request(get_with_host("/api/v1/server-info", "192.168.1.5:8080"))
                .await
                .status,
            200,
            "{}",
            t.label
        );
    }
}

#[tokio::test]
async fn responses_are_not_cached_and_document_is_served() {
    for t in apis(Listen::Network).await {
        let r = t.get("/api/v1/openapi.json", None).await;
        assert_eq!(r.headers["cache-control"], "no-store", "{}", t.label);
        assert_eq!(r.headers["x-content-type-options"], "nosniff");
        assert_eq!(r.body["openapi"], json!("3.1.0"));
        assert!(r.body["paths"]["/api/v1/server-info"]["get"].is_object());
        assert_eq!(
            r.body["components"]["schemas"]["ErrorBody"]["type"],
            json!("object")
        );
    }
}
