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
            (&o["warranty"], &o["warranty_term"]),
            (&json!("30 дней"), &json!({ "unit": "days", "value": 30 }))
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
        assert_eq!(patch(3, json!({ "warranty_days": 30 })).await.status, 400);
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
                    { "field": "final_price", "code": "required_for_issue", "message": "Заполните перед выдачей" },
                    { "field": "master", "code": "required_for_issue", "message": "Заполните перед выдачей" },
                    { "field": "work_done", "code": "required_for_issue", "message": "Заполните перед выдачей" }])
            )
        );
        assert_eq!(
            patch(
                3,
                json!({ "master": "Антон", "work_done": "Замена дисплея", "final_price": 100000 })
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

#[tokio::test]
async fn summary_values_and_new_filters() {
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
        create_order(
            &t,
            &token,
            new_order("Анна Ким", "8 911 000-00-00", "samsung", "A53"),
        )
        .await;
        let s = t.get("/api/v1/orders/summary", Some(&token)).await;
        assert_eq!(s.status, 200, "{}", t.label);
        assert_eq!(s.body["by_status"][0]["count"], json!(2));
        assert_eq!(
            (
                &s.body["need_call"],
                &s.body["need_parts"],
                &s.body["stale_ready"]
            ),
            (&json!(0), &json!(0), &json!(0))
        );
        let s = t
            .get("/api/v1/orders/summary?service_point_id=999", Some(&token))
            .await;
        assert_eq!(s.body["by_status"], json!([]));

        let v = t
            .get("/api/v1/orders/values?field=brand&query=sam", Some(&token))
            .await;
        assert_eq!(
            (v.status, &v.body),
            (200, &json!(["Samsung"])),
            "{}",
            t.label
        );
        let v = t
            .get(
                "/api/v1/orders/values?field=model&brand=SAMSUNG",
                Some(&token),
            )
            .await;
        assert_eq!(v.body.as_array().unwrap().len(), 2);
        let bad = t
            .get("/api/v1/orders/values?field=color", Some(&token))
            .await;
        assert_eq!(
            (bad.status, &bad.body["code"]),
            (400, &json!("bad_request"))
        );

        let list = t
            .get(
                "/api/v1/orders?brand=SAMSUNG&stale_ready=false",
                Some(&token),
            )
            .await;
        assert_eq!(ids(&list.body).len(), 2);
        let list = t
            .get(
                "/api/v1/orders?received_from=2026-11-11T09:00:00&received_to=2026-11-11T09:00:00",
                Some(&token),
            )
            .await;
        assert!(ids(&list.body).contains(&a), "{}", t.label);
    }
}

/// Заказ, готовый к выдаче: мастер и работа заполнены. Возвращает (номер, версия).
async fn order_ready_to_issue(t: &common::TestApi, token: &str) -> (i64, i64) {
    let id = create_order(t, token, new_order("Анна", "", "Samsung", "A52")).await["id"]
        .as_i64()
        .unwrap();
    let r = t
        .send(
            Method::PATCH,
            &format!("/api/v1/orders/{id}?version=1"),
            Some(token),
            Some(
                json!({ "master": "Антон", "work_done": "Замена дисплея", "final_price": 100000 }),
            ),
        )
        .await;
    assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
    (id, r.body["version"].as_i64().unwrap())
}

#[tokio::test]
async fn history_needs_only_login_and_pages_newest_first() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        let (id, _) = order_ready_to_issue(&t, &token).await;
        let clerk = t.token_with("clerk", &[Permission::OrdersEdit]).await;
        let r = t
            .get(
                &format!("/api/v1/orders/{id}/history?limit=1"),
                Some(&clerk),
            )
            .await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        assert_eq!(r.body["total"], json!(2));
        assert_eq!(r.body["items"].as_array().unwrap().len(), 1);
        assert_eq!(r.body["items"][0]["action"], json!("order.updated"));
        assert_eq!(
            t.get("/api/v1/orders/999999/history", Some(&clerk))
                .await
                .status,
            404
        );
        assert_eq!(
            t.get(
                &format!("/api/v1/orders/{id}/history?limit=0"),
                Some(&clerk)
            )
            .await
            .status,
            422
        );
        assert_eq!(
            t.get(&format!("/api/v1/orders/{id}/history"), None)
                .await
                .status,
            401
        );
    }
}

#[tokio::test]
async fn warranty_case_and_unstamped_issue() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        let (id, version) = order_ready_to_issue(&t, &token).await;
        let issued_status = status_id_by_name(&t.core, "Выдан").await;
        let r = t
            .send(
                Method::POST,
                &format!("/api/v1/orders/{id}/status?version={version}"),
                Some(&token),
                Some(json!({ "status_id": issued_status, "stamp_issued_at": false })),
            )
            .await;
        assert_eq!(
            (r.status, &r.body["issued_at"]),
            (200, &json!(null)),
            "{}: {}",
            t.label,
            r.body
        );
        let version = r.body["version"].as_i64().unwrap();
        let w = t
            .send(
                Method::POST,
                &format!("/api/v1/orders/{id}/warranty-case?version={version}"),
                Some(&token),
                Some(json!({ "complaint": "Снова не заряжается" })),
            )
            .await;
        assert_eq!(w.status, 200, "{}: {}", t.label, w.body);
        assert_eq!(
            w.body["status_id"],
            json!(status_id_by_name(&t.core, "Принят по гарантии").await)
        );
        let h = t
            .get(
                &format!("/api/v1/orders/{id}/history?limit=1"),
                Some(&token),
            )
            .await;
        assert_eq!(h.body["items"][0]["action"], json!("order.warranty_case"));
        let version = w.body["version"].as_i64().unwrap();
        let bad = t
            .send(
                Method::POST,
                &format!("/api/v1/orders/{id}/warranty-case?version={version}"),
                Some(&token),
                Some(json!({ "complaint": "x", "extra": 1 })),
            )
            .await;
        assert_eq!(bad.status, 400, "{}", t.label);
        let not_issued = t
            .send(
                Method::POST,
                &format!("/api/v1/orders/{id}/warranty-case?version={version}"),
                Some(&token),
                Some(json!({ "complaint": "x" })),
            )
            .await;
        assert_eq!(
            (not_issued.status, &not_issued.body["fields"][0]["message"]),
            (
                422,
                &json!("Принять по гарантии можно только выданный заказ")
            )
        );
    }
}

/// Повтор `POST /orders` с тем же `request_key` — тот же заказ и заголовок `X-MW-Replayed: 1`;
/// первая попытка заголовка не несёт.
#[tokio::test]
async fn replayed_order_creation_is_marked() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        let mut body = new_order("Анна", "", "Samsung", "A52");
        body["request_key"] = json!("replay-1");
        let first = t
            .send(
                Method::POST,
                "/api/v1/orders",
                Some(&token),
                Some(body.clone()),
            )
            .await;
        assert_eq!(first.status, 201, "{}", t.label);
        assert!(first.headers.get("x-mw-replayed").is_none(), "{}", t.label);
        let again = t
            .send(Method::POST, "/api/v1/orders", Some(&token), Some(body))
            .await;
        assert_eq!(again.status, 201, "{}", t.label);
        assert_eq!(again.headers["x-mw-replayed"], "1", "{}", t.label);
        assert_eq!(again.body["id"], first.body["id"]);
        let plain = create_order(&t, &token, new_order("Анна", "", "Samsung", "A52")).await;
        assert_ne!(plain["id"], first.body["id"]);
    }
}

#[tokio::test]
async fn history_before_id_gives_only_older_entries() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        let (id, _) = order_ready_to_issue(&t, &token).await;
        let all = t
            .get(&format!("/api/v1/orders/{id}/history"), Some(&token))
            .await;
        let newest = all.body["items"][0]["id"].as_i64().unwrap();
        let r = t
            .get(
                &format!("/api/v1/orders/{id}/history?before_id={newest}"),
                Some(&token),
            )
            .await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        let items = r.body["items"].as_array().unwrap();
        assert_eq!(items.len(), 1, "{}", t.label);
        assert_eq!(items[0]["action"], json!("order.created"));
        assert_eq!(r.body["total"], json!(2));
    }
}
