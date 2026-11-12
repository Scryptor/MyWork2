//! Вход, сессии, режим без входа, ограничение попыток.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::body::Body;
use axum::http::{Method, Request};
use chrono::Duration;
use common::{Listen, TEST_PASSWORD, apis};
use serde_json::json;

#[tokio::test]
async fn login_me_logout() {
    for t in apis(Listen::Network).await {
        let irina = t.create_admin_user("Ирина").await;
        let r = t
            .send(
                Method::POST,
                "/api/v1/auth/login",
                None,
                Some(json!({ "login": " ирина ", "password": TEST_PASSWORD, "device_label": "Касса" })),
            )
            .await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        let token = r.body["token"].as_str().unwrap().to_owned();
        assert_eq!((token.len(), &r.body["user"]["id"]), (64, &json!(irina.id)));
        assert!(
            r.body["permissions"]
                .as_array()
                .unwrap()
                .contains(&json!("users_manage"))
        );
        assert!(r.body["user"].get("password_hash").is_none());
        let me = t.get("/api/v1/auth/me", Some(&token)).await;
        assert_eq!(
            me.body,
            json!({ "user_id": irina.id, "name": "Ирина", "permissions": r.body["permissions"], "open_mode": false })
        );
        assert_eq!(
            t.send(Method::POST, "/api/v1/auth/logout", Some(&token), None)
                .await
                .status,
            204
        );
        let r = t.get("/api/v1/auth/me", Some(&token)).await;
        assert_eq!(
            (r.status, &r.body["code"]),
            (401, &json!("unauthenticated"))
        );
        assert_eq!(r.headers["www-authenticate"], "Bearer");
    }
}

#[tokio::test]
async fn logout_without_or_with_malformed_header_is_204() {
    for t in apis(Listen::Network).await {
        assert_eq!(
            t.send(Method::POST, "/api/v1/auth/logout", None, None)
                .await
                .status,
            204,
            "{}",
            t.label
        );
        let req = Request::post("/api/v1/auth/logout")
            .header("host", "127.0.0.1")
            .header("content-type", "application/json")
            .header("authorization", "Basic abc")
            .body(Body::empty())
            .unwrap();
        assert_eq!(t.request(req).await.status, 204);
    }
}

#[tokio::test]
async fn malformed_authorization_header_is_unauthenticated() {
    for t in apis(Listen::Loopback).await {
        for value in ["Basic abc", "Bearer", "abc"] {
            let req = Request::get("/api/v1/auth/me")
                .header("host", "127.0.0.1")
                .header("authorization", value)
                .body(Body::empty())
                .unwrap();
            let r = t.request(req).await;
            assert_eq!(
                (r.status, &r.body["code"]),
                (401, &json!("unauthenticated")),
                "{}: {value}",
                t.label
            );
        }
    }
}

#[tokio::test]
async fn wrong_password_and_unknown_login_look_the_same() {
    for t in apis(Listen::Network).await {
        t.create_admin_user("Ирина").await;
        let a = t
            .send(
                Method::POST,
                "/api/v1/auth/login",
                None,
                Some(json!({ "login": "ирина", "password": "не тот", "device_label": "" })),
            )
            .await;
        let b = t
            .send(
                Method::POST,
                "/api/v1/auth/login",
                None,
                Some(json!({ "login": "никто", "password": "не тот", "device_label": "" })),
            )
            .await;
        assert_eq!((a.status, &a.body), (401, &b.body), "{}", t.label);
        assert_eq!(a.body["code"], json!("invalid_credentials"));
    }
}

#[tokio::test]
async fn open_mode_only_on_loopback_and_only_without_users() {
    for t in apis(Listen::Loopback).await {
        let me = t.get("/api/v1/auth/me", None).await;
        assert_eq!(
            (me.status, &me.body["name"], &me.body["open_mode"]),
            (200, &json!("Без входа"), &json!(true)),
            "{}",
            t.label
        );
        assert_eq!(me.body["permissions"].as_array().unwrap().len(), 17);
        assert_eq!(
            t.get("/api/v1/auth/me", Some(&"0".repeat(64))).await.status,
            401
        ); // неверный токен не превращается в режим без входа
        t.create_admin_user("boss").await;
        assert_eq!(t.get("/api/v1/auth/me", None).await.status, 401);
    }
    for t in apis(Listen::Network).await {
        assert_eq!(
            t.get("/api/v1/auth/me", None).await.status,
            401,
            "{}",
            t.label
        );
    }
}

#[tokio::test]
async fn login_is_limited_per_login_and_per_address() {
    for t in apis(Listen::Network).await {
        t.create_admin_user("Ирина").await;
        let login = |pass: &str| json!({ "login": "ирина", "password": pass, "device_label": "" });
        for _ in 0..5 {
            assert_eq!(
                t.send_from(
                    [10, 0, 0, 7],
                    Method::POST,
                    "/api/v1/auth/login",
                    None,
                    Some(login("не тот"))
                )
                .await
                .status,
                401
            );
        }
        let r = t
            .send_from(
                [10, 0, 0, 8],
                Method::POST,
                "/api/v1/auth/login",
                None,
                Some(login(TEST_PASSWORD)),
            )
            .await;
        assert_eq!(
            (r.status, &r.body["code"], &r.body["retry_after_secs"]),
            (429, &json!("too_many_attempts"), &json!(900)),
            "{}",
            t.label
        );
        assert_eq!(r.headers["retry-after"], "900");
        t.clock.advance(Duration::minutes(15));
        assert_eq!(
            t.send_from(
                [10, 0, 0, 8],
                Method::POST,
                "/api/v1/auth/login",
                None,
                Some(login(TEST_PASSWORD))
            )
            .await
            .status,
            200
        );
        for i in 0..20 {
            let body = json!({ "login": format!("user{i}"), "password": "x", "device_label": "" });
            assert_eq!(
                t.send_from(
                    [10, 0, 0, 9],
                    Method::POST,
                    "/api/v1/auth/login",
                    None,
                    Some(body)
                )
                .await
                .status,
                401
            );
        }
        assert_eq!(
            t.send_from(
                [10, 0, 0, 9],
                Method::POST,
                "/api/v1/auth/login",
                None,
                Some(login(TEST_PASSWORD))
            )
            .await
            .status,
            429
        );
        assert_eq!(
            t.send_from(
                [10, 0, 0, 10],
                Method::POST,
                "/api/v1/auth/login",
                None,
                Some(login(TEST_PASSWORD))
            )
            .await
            .status,
            200
        );
    }
}

#[tokio::test]
async fn password_change_is_limited_and_revokes_sessions() {
    for t in apis(Listen::Network).await {
        t.create_admin_user("Ирина").await;
        let token = t.login("Ирина").await;
        let change = |cur: &str| json!({ "current_password": cur, "new_password": "новый-пароль" });
        for _ in 0..5 {
            let r = t
                .send(
                    Method::POST,
                    "/api/v1/auth/password",
                    Some(&token),
                    Some(change("не тот")),
                )
                .await;
            assert_eq!(
                (r.status, &r.body["fields"][0]["field"]),
                (422, &json!("current_password"))
            );
        }
        assert_eq!(
            t.send(
                Method::POST,
                "/api/v1/auth/password",
                Some(&token),
                Some(change(TEST_PASSWORD))
            )
            .await
            .status,
            429
        );
        t.clock.advance(Duration::minutes(15));
        assert_eq!(
            t.send(
                Method::POST,
                "/api/v1/auth/password",
                Some(&token),
                Some(change(TEST_PASSWORD))
            )
            .await
            .status,
            204
        );
        assert_eq!(t.get("/api/v1/auth/me", Some(&token)).await.status, 401);
    }
}

#[tokio::test]
async fn malformed_login_requests_are_bad_requests() {
    for t in apis(Listen::Network).await {
        for body in [
            json!({ "login": "x" }),
            json!({ "login": "x", "password": "y", "device_label": "", "extra": 1 }),
            json!({ "login": 5, "password": "y", "device_label": "" }),
        ] {
            let r = t
                .send(Method::POST, "/api/v1/auth/login", None, Some(body))
                .await;
            assert_eq!(
                (r.status, &r.body["code"]),
                (400, &json!("bad_request")),
                "{}",
                t.label
            );
        }
        let broken = Request::post("/api/v1/auth/login")
            .header("host", "127.0.0.1")
            .header("content-type", "application/json")
            .body(Body::from("{"))
            .unwrap();
        assert_eq!(t.request(broken).await.status, 400);
    }
}

#[tokio::test]
async fn concurrent_wrong_passwords_cannot_exceed_the_limit() {
    for t in apis(Listen::Network).await {
        t.create_admin_user("Ирина").await;
        let wrong = json!({ "login": "ирина", "password": "не тот", "device_label": "" });
        let responses = futures::future::join_all((0..10).map(|_| {
            t.send_from(
                [10, 0, 0, 7],
                Method::POST,
                "/api/v1/auth/login",
                None,
                Some(wrong.clone()),
            )
        }))
        .await;
        let count = |code: u16| responses.iter().filter(|r| r.status == code).count();
        assert_eq!((count(401), count(429)), (5, 5), "{}", t.label);
        let ok = json!({ "login": "ирина", "password": TEST_PASSWORD, "device_label": "" });
        let r = t
            .send_from(
                [10, 0, 0, 8],
                Method::POST,
                "/api/v1/auth/login",
                None,
                Some(ok),
            )
            .await;
        assert_eq!(r.status, 429, "{}", t.label);
    }
}

#[tokio::test]
async fn short_new_password_names_the_request_field() {
    for t in apis(Listen::Network).await {
        t.create_admin_user("Ирина").await;
        let token = t.login("Ирина").await;
        let r = t
            .send(
                Method::POST,
                "/api/v1/auth/password",
                Some(&token),
                Some(json!({ "current_password": TEST_PASSWORD, "new_password": "12" })),
            )
            .await;
        assert_eq!(r.status, 422, "{}: {}", t.label, r.body);
        assert_eq!(r.body["fields"][0]["field"], "new_password", "{}", t.label);
    }
}
