//! Настройки, статусы, справочники, точки обслуживания.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::Method;
use common::{Listen, apis, status_id_by_name};
use mw_domain::permission::Permission;
use serde_json::json;

#[tokio::test]
async fn settings_endpoints() {
    for t in apis(Listen::Network).await {
        let admin = t.admin_token().await;
        let clerk = t.token_with("kassa", &[Permission::OrdersCreate]).await;
        let r = t.get("/api/v1/settings", Some(&clerk)).await;
        assert_eq!(
            (
                r.status,
                &r.body["currency_symbol"],
                &r.body["default_warranty"]
            ),
            (200, &json!("₽"), &json!("30 дней")),
            "{}",
            t.label
        );
        let r = t
            .send(
                Method::PATCH,
                "/api/v1/settings",
                Some(&admin),
                Some(json!({ "default_warranty": "6 месяцев" })),
            )
            .await;
        assert_eq!(
            (r.status, &r.body["default_warranty"]),
            (200, &json!("6 месяцев"))
        );
        let r = t
            .send(
                Method::PATCH,
                "/api/v1/settings",
                Some(&admin),
                Some(json!({ "default_warranty_days": 30 })),
            )
            .await;
        assert_eq!(r.status, 400, "{}", t.label);
        let r = t
            .send(
                Method::PATCH,
                "/api/v1/settings",
                Some(&clerk),
                Some(json!({ "master_percent": 40 })),
            )
            .await;
        assert_eq!(
            (r.status, &r.body["code"], &r.body["permission"]),
            (403, &json!("forbidden"), &json!("settings_manage"))
        );
        let r = t
            .send(
                Method::PATCH,
                "/api/v1/settings",
                Some(&admin),
                Some(json!({ "master_percent": 101 })),
            )
            .await;
        assert_eq!(
            (r.status, &r.body["fields"]),
            (
                422,
                &json!([{ "field": "master_percent", "code": "out_of_range", "message": "Значение вне допустимого диапазона" }])
            )
        );
        assert_eq!(
            t.send(
                Method::PATCH,
                "/api/v1/settings",
                Some(&admin),
                Some(json!({ "master_percent": null }))
            )
            .await
            .status,
            400
        );
    }
}

#[tokio::test]
async fn status_endpoints() {
    for t in apis(Listen::Network).await {
        let admin = t.admin_token().await;
        assert_eq!(
            t.get("/api/v1/statuses", Some(&admin))
                .await
                .body
                .as_array()
                .unwrap()
                .len(),
            8
        );
        let r = t
            .send(
                Method::POST,
                "/api/v1/statuses",
                Some(&admin),
                Some(json!({ "name": "Выдан курьером", "kind": "issued", "color": "gray" })),
            )
            .await;
        assert_eq!((r.status, &r.body["kind"]), (201, &json!("issued")));
        let issued = status_id_by_name(&t.core, "Выдан").await;
        let r = t
            .send(
                Method::PATCH,
                &format!("/api/v1/statuses/{issued}"),
                Some(&admin),
                Some(json!({ "archived": true })),
            )
            .await;
        assert_eq!((r.status, &r.body["archived"]), (200, &json!(true)));
        assert_eq!(
            t.get("/api/v1/statuses", Some(&admin))
                .await
                .body
                .as_array()
                .unwrap()
                .len(),
            8
        );
        let all = t
            .get("/api/v1/statuses?include_archived=true", Some(&admin))
            .await
            .body;
        let ids: Vec<i64> = all
            .as_array()
            .unwrap()
            .iter()
            .rev()
            .map(|s| s["id"].as_i64().unwrap())
            .collect();
        assert_eq!(ids.len(), 9);
        assert_eq!(
            t.send(
                Method::PUT,
                "/api/v1/statuses/order",
                Some(&admin),
                Some(json!({ "ordered_ids": ids }))
            )
            .await
            .status,
            204
        );
        assert_eq!(
            t.get("/api/v1/statuses?include_archived=true", Some(&admin))
                .await
                .body[0]["id"],
            json!(ids[0])
        );
        let r = t
            .send(
                Method::PUT,
                "/api/v1/statuses/order",
                Some(&admin),
                Some(json!({ "ordered_ids": &ids[1..] })),
            )
            .await;
        assert_eq!(
            (r.status, &r.body["fields"][0]["field"]),
            (422, &json!("ordered_ids"))
        );
        let r = t
            .send(
                Method::PATCH,
                "/api/v1/statuses/999999",
                Some(&admin),
                Some(json!({ "color": "red" })),
            )
            .await;
        assert_eq!((r.status, &r.body["code"]), (404, &json!("not_found")));
        assert!(r.body["message"].as_str().unwrap().contains("статус"));
    }
}

#[tokio::test]
async fn dictionary_endpoints() {
    for t in apis(Listen::Network).await {
        let admin = t.admin_token().await;
        let clerk = t.token_with("kassa", &[Permission::OrdersCreate]).await;
        assert!(
            t.get("/api/v1/dictionaries/brand", Some(&clerk))
                .await
                .body
                .as_array()
                .unwrap()
                .iter()
                .any(|i| i["value"] == "Apple")
        );
        let r = t
            .send(
                Method::POST,
                "/api/v1/dictionaries/brand",
                Some(&clerk),
                Some(json!({ "value": "  Nothing " })),
            )
            .await;
        assert_eq!(
            (r.status, &r.body["value"], &r.body["kind"]),
            (201, &json!("Nothing"), &json!("brand"))
        );
        let id = r.body["id"].as_i64().unwrap();
        assert_eq!(
            t.send(
                Method::DELETE,
                &format!("/api/v1/dictionary-items/{id}"),
                Some(&clerk),
                None
            )
            .await
            .status,
            403
        );
        let r = t
            .send(
                Method::PATCH,
                &format!("/api/v1/dictionary-items/{id}"),
                Some(&admin),
                Some(json!({ "value": "Nothing Phone" })),
            )
            .await;
        assert_eq!(r.body["value"], json!("Nothing Phone"));
        assert_eq!(
            t.send(
                Method::DELETE,
                &format!("/api/v1/dictionary-items/{id}"),
                Some(&admin),
                None
            )
            .await
            .status,
            204
        );
        let colors: Vec<i64> = t
            .get("/api/v1/dictionaries/color", Some(&admin))
            .await
            .body
            .as_array()
            .unwrap()
            .iter()
            .rev()
            .map(|i| i["id"].as_i64().unwrap())
            .collect();
        assert_eq!(
            t.send(
                Method::PUT,
                "/api/v1/dictionaries/color/order",
                Some(&admin),
                Some(json!({ "ordered_ids": colors }))
            )
            .await
            .status,
            204
        );
        assert_eq!(
            t.get("/api/v1/dictionaries/color", Some(&admin)).await.body[0]["id"],
            json!(colors[0])
        );
        assert_eq!(
            t.get("/api/v1/dictionaries/planet", Some(&admin))
                .await
                .status,
            400
        );
    }
}

#[tokio::test]
async fn service_point_endpoints() {
    for t in apis(Listen::Network).await {
        let admin = t.admin_token().await;
        let r = t
            .send(
                Method::POST,
                "/api/v1/service-points",
                Some(&admin),
                Some(json!({ "name": "Древлянка" })),
            )
            .await;
        assert_eq!(r.status, 201);
        let id = r.body["id"].as_i64().unwrap();
        let r = t
            .send(
                Method::PATCH,
                &format!("/api/v1/service-points/{id}"),
                Some(&admin),
                Some(json!({ "archived": true })),
            )
            .await;
        assert_eq!(r.body["archived"], json!(true));
        assert_eq!(
            t.get("/api/v1/service-points", Some(&admin)).await.body,
            json!([])
        );
        assert_eq!(
            t.get("/api/v1/service-points?include_archived=true", Some(&admin))
                .await
                .body
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            t.send(
                Method::PUT,
                "/api/v1/service-points/order",
                Some(&admin),
                Some(json!({ "ordered_ids": [id] }))
            )
            .await
            .status,
            204
        );
    }
}

#[tokio::test]
async fn status_color_outside_the_palette_is_a_validation_error() {
    for t in apis(Listen::Network).await {
        let admin = t.admin_token().await;
        let r = t
            .send(
                Method::POST,
                "/api/v1/statuses",
                Some(&admin),
                Some(json!({ "name": "Цветной", "kind": "open", "color": "#ff0000" })),
            )
            .await;
        assert_eq!(
            (r.status, &r.body["fields"][0]),
            (
                422,
                &json!({ "field": "color", "code": "invalid",
                         "message": "Цвет статуса — одно из 12 имён палитры" })
            ),
            "{}",
            t.label
        );
    }
}
