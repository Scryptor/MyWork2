//! Первый запуск: `setup_required` в `server-info` и `POST /api/v1/setup`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::Method;
use common::{Listen, apis, create_order, new_order};
use mw_domain::permission::Permission;
use serde_json::json;

fn body() -> serde_json::Value {
    json!({ "firm_name": "Сервис", "point_name": "Центр", "masters": ["Иван", "иван", "Пётр"] })
}

#[tokio::test]
async fn setup_flow() {
    for t in apis(Listen::Network).await {
        let admin = t.admin_token().await;
        let info = t.get("/api/v1/server-info", None).await;
        assert_eq!(info.body["setup_required"], json!(true), "{}", t.label);

        let r = t
            .send(Method::POST, "/api/v1/setup", Some(&admin), Some(body()))
            .await;
        assert_eq!(r.status, 204, "{} {:?}", t.label, r.body);
        let info = t.get("/api/v1/server-info", None).await;
        assert_eq!(info.body["setup_required"], json!(false), "{}", t.label);

        let settings = t.get("/api/v1/settings", Some(&admin)).await.body;
        assert_eq!(settings["firm_name"], json!("Сервис"), "{}", t.label);
        assert!(settings["default_service_point_id"].is_i64(), "{}", t.label);
        let masters = t
            .get("/api/v1/dictionaries/master", Some(&admin))
            .await
            .body;
        assert_eq!(
            masters.as_array().unwrap().len(),
            2,
            "{} {masters}",
            t.label
        );

        let again = t
            .send(Method::POST, "/api/v1/setup", Some(&admin), Some(body()))
            .await;
        assert_eq!(again.status, 409, "{}", t.label);
    }
}

#[tokio::test]
async fn setup_checks_rights_and_input() {
    for t in apis(Listen::Network).await {
        let clerk = t.token_with("kassa", &[Permission::OrdersCreate]).await;
        let r = t
            .send(Method::POST, "/api/v1/setup", Some(&clerk), Some(body()))
            .await;
        assert_eq!(r.status, 403, "{}", t.label);
        let admin = t.admin_token().await;
        let r = t
            .send(
                Method::POST,
                "/api/v1/setup",
                Some(&admin),
                Some(json!({ "firm_name": "", "point_name": "Центр", "masters": [] })),
            )
            .await;
        assert_eq!(r.status, 422, "{}", t.label);
        let r = t
            .send(
                Method::POST,
                "/api/v1/setup",
                Some(&admin),
                Some(json!({ "firm_name": "С", "point_name": "Ц", "masters": [], "x": 1 })),
            )
            .await;
        assert_eq!(r.status, 400, "{} {:?}", t.label, r.body);
    }
}

#[tokio::test]
async fn setup_is_not_required_with_an_existing_order() {
    for t in apis(Listen::Network).await {
        let admin = t.admin_token().await;
        create_order(&t, &admin, new_order("Анна", "", "Samsung", "A52")).await;
        let info = t.get("/api/v1/server-info", None).await;
        assert_eq!(info.body["setup_required"], json!(false), "{}", t.label);
    }
}
