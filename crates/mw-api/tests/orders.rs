//! Заказы: приём, список, правки с версиями, статус, выдача, удаление, нумерация.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::Method;
use chrono::Duration;
use common::{Listen, apis, create_order, ids, new_order, status_id_by_name};
use mw_domain::barcode::order_barcode;
use mw_domain::permission::Permission;
use serde_json::{Value, json};

#[tokio::test]
async fn intake_and_read() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        let mut body = new_order("Пётр Иванов", "+7 921 123-45-67", "Samsung", "A52");
        body["prepayment"] = json!(200_000);
        let o = create_order(&t, &token, body).await;
        let id = o["id"].as_i64().unwrap();
        assert_eq!(
            o["barcode"],
            json!(order_barcode(id).unwrap()),
            "{}",
            t.label
        );
        assert_eq!(
            (&o["received_at"], &o["prepaid_at"]),
            (&json!("2026-11-11T09:00:00"), &json!("2026-11-11T09:00:00"))
        );
        assert_eq!(
            t.get(&format!("/api/v1/orders/{id}"), Some(&token))
                .await
                .body,
            o
        );
        let mut bad = new_order("x", "", "Samsung", "A52");
        bad["client"] = json!({ "existing": 999_999 });
        let r = t
            .send(Method::POST, "/api/v1/orders", Some(&token), Some(bad))
            .await;
        assert_eq!(
            (r.status, &r.body["fields"][0]["field"]),
            (422, &json!("client_id"))
        );
    }
}

#[tokio::test]
async fn list_with_query_parameters() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        let a = create_order(
            &t,
            &token,
            new_order("Пётр Иванов", "+7 921 123-45-67", "Samsung", "A52"),
        )
        .await["id"]
            .as_i64()
            .unwrap();
        t.clock.advance(Duration::hours(1));
        let b = create_order(
            &t,
            &token,
            new_order("Анна Ким", "8 911 000-00-00", "Apple", "MacBook"),
        )
        .await["id"]
            .as_i64()
            .unwrap();
        let ready = status_id_by_name(&t.core, "Готов").await;
        assert_eq!(
            t.send(
                Method::POST,
                &format!("/api/v1/orders/{b}/status?version=1"),
                Some(&token),
                Some(json!({ "status_id": ready }))
            )
            .await
            .status,
            200
        );
        let list = async |q: &str| t.get(&format!("/api/v1/orders{q}"), Some(&token)).await;
        assert_eq!(ids(&list("").await.body), vec![b, a], "{}", t.label);
        assert_eq!(
            ids(&list("?query=%D0%BF%D0%B5%D1%82%D1%80%20a52").await.body),
            vec![a]
        ); // «петр a52»
        assert_eq!(
            ids(&list(&format!("?status_ids={ready}&status_ids=999"))
                .await
                .body),
            vec![b]
        );
        assert_eq!(
            ids(&list("?issued=in_service&sort=client_name").await.body),
            vec![b, a]
        ); // анна, петр
        assert_eq!(
            ids(&list("?sort=client_name&descending=true").await.body),
            vec![a, b]
        );
        let page = list("?limit=1&offset=1").await.body;
        assert_eq!((&page["total"], ids(&page)), (&json!(2), vec![a]));
        assert_eq!(list("?issued=bogus").await.status, 400);
        let clerk = t.token_with("kassa", &[Permission::OrdersCreate]).await;
        let r = t
            .get("/api/v1/orders?include_deleted=true", Some(&clerk))
            .await;
        assert_eq!(
            (r.status, &r.body["permission"]),
            (403, &json!("orders_delete"))
        );
        let barcode = t
            .get(&format!("/api/v1/orders/{a}"), Some(&token))
            .await
            .body["barcode"]
            .as_str()
            .unwrap()
            .to_owned();
        let found = t
            .get(
                &format!("/api/v1/orders/by-barcode?code=0{barcode}%0D%0A"),
                Some(&token),
            )
            .await
            .body;
        assert_eq!(
            found
                .as_array()
                .unwrap()
                .iter()
                .map(|o| o["id"].as_i64().unwrap())
                .collect::<Vec<_>>(),
            vec![a]
        );
    }
}

#[tokio::test]
async fn edits_status_issue_delete_with_versions() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        let id = create_order(&t, &token, new_order("Анна", "", "Samsung", "A52")).await["id"]
            .as_i64()
            .unwrap();
        let patch = async |v: i64, body: Value| {
            t.send(
                Method::PATCH,
                &format!("/api/v1/orders/{id}?version={v}"),
                Some(&token),
                Some(body),
            )
            .await
        };
        let r = patch(1, json!({ "brand": "Apple", "estimate_from": 100_000 })).await;
        assert_eq!(
            (r.status, &r.body["version"], &r.body["estimate_from"]),
            (200, &json!(2), &json!(100000)),
            "{}",
            t.label
        );
        let r = patch(2, json!({ "estimate_from": null })).await;
        assert_eq!(
            (&r.body["version"], &r.body["estimate_from"]),
            (&json!(3), &json!(null))
        );
        assert_eq!(patch(3, json!({ "brand": null })).await.status, 400);
        assert_eq!(patch(3, json!({ "brnad": "Apple" })).await.status, 400);
        let r = patch(1, json!({ "model": "X" })).await;
        assert_eq!((r.status, &r.body["current_version"]), (409, &json!(3)));
        assert_eq!(
            patch(1, json!({ "brand": "Apple" })).await.body["version"],
            json!(3)
        );
        let r = t
            .send(
                Method::POST,
                &format!("/api/v1/orders/{id}/issue?version=3"),
                Some(&token),
                None,
            )
            .await;
        assert_eq!(
            (r.status, &r.body["fields"]),
            (
                422,
                &json!([
                    { "field": "master", "code": "required", "message": "Заполните поле" },
                    { "field": "work_done", "code": "required", "message": "Заполните поле" }])
            )
        );
        assert_eq!(
            patch(
                3,
                json!({ "master": "Антон", "work_done": "Замена дисплея" })
            )
            .await
            .body["version"],
            json!(4)
        );
        let r = t
            .send(
                Method::POST,
                &format!("/api/v1/orders/{id}/issue?version=4"),
                Some(&token),
                None,
            )
            .await;
        assert_eq!(
            (r.status, &r.body["issued_at"]),
            (200, &json!("2026-11-11T09:00:00"))
        );
        assert_eq!(
            t.send(
                Method::DELETE,
                &format!("/api/v1/orders/{id}?version=5"),
                Some(&token),
                None
            )
            .await
            .status,
            204
        );
        assert!(
            !t.get(&format!("/api/v1/orders/{id}"), Some(&token))
                .await
                .body["deleted_at"]
                .is_null()
        );
    }
}

#[tokio::test]
async fn next_order_number() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        let id = create_order(&t, &token, new_order("Анна", "", "Samsung", "A52")).await["id"]
            .as_i64()
            .unwrap();
        let r = t
            .send(
                Method::PUT,
                "/api/v1/orders/next-number",
                Some(&token),
                Some(json!({ "next": id })),
            )
            .await;
        assert_eq!(
            (r.status, &r.body["fields"][0]["field"]),
            (422, &json!("next"))
        );
        assert_eq!(
            t.send(
                Method::PUT,
                "/api/v1/orders/next-number",
                Some(&token),
                Some(json!({ "next": id + 100 }))
            )
            .await
            .status,
            204
        );
        assert_eq!(
            create_order(&t, &token, new_order("Олег", "", "Xiaomi", "Note 10")).await["id"],
            json!(id + 100)
        );
    }
}
