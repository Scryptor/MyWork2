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

const ALL: [Permission; 3] = [
    Permission::StockView,
    Permission::StockEdit,
    Permission::StockCreate,
];

/// Шкаф `B` 3×5 и позиция с остатком 10: номера шкафа и позиции.
async fn rack_and_item(t: &common::TestApi, token: &str) -> (i64, i64) {
    let r = t
        .send(
            Method::POST,
            "/api/v1/stock/racks",
            Some(token),
            Some(body("B")),
        )
        .await;
    let rack = r.body["id"].as_i64().unwrap();
    let r = t
        .send(
            Method::POST,
            "/api/v1/stock",
            Some(token),
            Some(json!({ "name": "Дисплей", "quantity": 10 })),
        )
        .await;
    assert_eq!(r.status, 201, "{}: {}", t.label, r.body);
    (rack, r.body["id"].as_i64().unwrap())
}

#[tokio::test]
async fn places_round_trip() {
    for t in apis(Listen::Network).await {
        let token = t.token_with("sklad", &ALL).await;
        let (rack, item) = rack_and_item(&t, &token).await;
        let r = t
            .send(
                Method::POST,
                &format!("/api/v1/stock/{item}/places"),
                Some(&token),
                Some(json!({ "place": { "rack_id": rack, "shelf": 2, "cell": 4 }, "quantity": 6 })),
            )
            .await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        assert_eq!(r.body["places"][0]["code"], json!("B-2-4"));
        assert_eq!(r.body["unplaced"], json!(4));
        let r = t
            .send(
                Method::POST,
                &format!("/api/v1/stock/{item}/places/move"),
                Some(&token),
                Some(json!({
                    "from": { "rack_id": rack, "shelf": 2, "cell": 4 },
                    "to": { "rack_id": rack, "shelf": 1, "cell": 1 },
                    "quantity": 2
                })),
            )
            .await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        assert_eq!(r.body["places"].as_array().unwrap().len(), 2);
        let r = t
            .send(
                Method::DELETE,
                &format!("/api/v1/stock/{item}/places/{rack}/2/4"),
                Some(&token),
                None,
            )
            .await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        assert_eq!(r.body["unplaced"], json!(8));
        assert_eq!(r.body["places"].as_array().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn an_occupied_rack_answers_409_with_its_cells() {
    for t in apis(Listen::Network).await {
        let token = t.token_with("sklad", &ALL).await;
        let (rack, item) = rack_and_item(&t, &token).await;
        let r = t
            .send(
                Method::POST,
                &format!("/api/v1/stock/{item}/places"),
                Some(&token),
                Some(json!({ "place": { "rack_id": rack, "shelf": 3, "cell": 2 }, "quantity": 1 })),
            )
            .await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        let r = t
            .send(
                Method::PATCH,
                &format!("/api/v1/stock/racks/{rack}"),
                Some(&token),
                Some(json!({ "shelves": 2 })),
            )
            .await;
        assert_eq!(r.status, 409, "{}: {}", t.label, r.body);
        assert!(
            r.body
                .to_string()
                .contains("В ячейке B-3-2 лежат запчасти — переложите их"),
            "{}",
            r.body
        );
        let r = t
            .send(
                Method::DELETE,
                &format!("/api/v1/stock/racks/{rack}"),
                Some(&token),
                None,
            )
            .await;
        assert_eq!(r.status, 409, "{}: {}", t.label, r.body);
    }
}

#[tokio::test]
async fn places_exceed_is_422() {
    for t in apis(Listen::Network).await {
        let token = t.token_with("sklad", &ALL).await;
        let (rack, item) = rack_and_item(&t, &token).await;
        let r = t
            .send(
                Method::POST,
                &format!("/api/v1/stock/{item}/places"),
                Some(&token),
                Some(
                    json!({ "place": { "rack_id": rack, "shelf": 1, "cell": 1 }, "quantity": 11 }),
                ),
            )
            .await;
        assert_eq!(r.status, 422, "{}: {}", t.label, r.body);
        assert!(r.body.to_string().contains("places_exceed"), "{}", r.body);
    }
}

#[tokio::test]
async fn stock_search_knows_place_codes() {
    for t in apis(Listen::Network).await {
        let token = t.token_with("sklad", &ALL).await;
        let (rack, item) = rack_and_item(&t, &token).await;
        let r = t
            .send(
                Method::POST,
                &format!("/api/v1/stock/{item}/places"),
                Some(&token),
                Some(json!({ "place": { "rack_id": rack, "shelf": 2, "cell": 4 }, "quantity": 6 })),
            )
            .await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);

        let r = t.get("/api/v1/stock?query=%D0%B8-2-4", Some(&token)).await;
        assert_eq!(
            (r.status, &r.body["place"]),
            (200, &json!("B-2-4")),
            "{}: {}",
            t.label,
            r.body
        );
        let r = t
            .get("/api/v1/stock?query=B-2-4&as_text=true", Some(&token))
            .await;
        assert_eq!(
            (r.status, &r.body["place"], &r.body["items"]),
            (200, &json!(null), &json!([])),
            "{}: {}",
            t.label,
            r.body
        );
        let r = t.get("/api/v1/stock?place=nope", Some(&token)).await;
        assert_eq!(r.status, 422, "{}: {}", t.label, r.body);
        let text = r.body.to_string();
        assert!(
            text.contains("place") && text.contains("no_such_place"),
            "{}: {text}",
            t.label
        );
        let r = t.get("/api/v1/stock/racks?query=b-2-4", Some(&token)).await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        let cells = r.body[0]["cells"].as_array().unwrap();
        let found: Vec<&str> = cells
            .iter()
            .filter(|c| c["found"] == json!(true))
            .map(|c| c["code"].as_str().unwrap())
            .collect();
        assert_eq!(found, vec!["B-2-4"], "{}", t.label);
    }
}

#[tokio::test]
async fn stickers_come_as_pdf_or_png() {
    for t in apis(Listen::Network).await {
        let token = t
            .token_with("sklad", &[Permission::StockView, Permission::StockEdit])
            .await;
        let r = t
            .send(
                Method::POST,
                "/api/v1/stock/racks",
                Some(&token),
                Some(body("b")),
            )
            .await;
        let id = r.body["id"].as_i64().unwrap();

        let (status, headers, bytes) = t
            .get_raw(
                &format!("/api/v1/stock/racks/{id}/cells/2/4/sticker?format=pdf"),
                &token,
            )
            .await;
        assert_eq!(status, 200, "{}", t.label);
        assert_eq!(headers["content-type"], "application/pdf");
        assert_eq!(
            headers["content-disposition"], "inline; filename=\"naklejka-yachejki-B-2-4.pdf\"",
            "{}",
            t.label
        );
        assert!(bytes.starts_with(b"%PDF-"));

        let r = t
            .get(
                &format!("/api/v1/stock/racks/{id}/cells/2/4/sticker?format=png"),
                Some(&token),
            )
            .await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        assert_eq!(r.body["pages"].as_array().unwrap().len(), 1);

        let (status, headers, bytes) = t
            .get_raw(
                &format!("/api/v1/stock/racks/{id}/stickers?format=pdf"),
                &token,
            )
            .await;
        assert_eq!(status, 200, "{}", t.label);
        assert_eq!(
            headers["content-disposition"],
            "inline; filename=\"naklejki-shkafa-B.pdf\""
        );
        assert!(bytes.starts_with(b"%PDF-"));

        let r = t
            .get(
                &format!("/api/v1/stock/racks/{id}/cells/9/9/sticker"),
                Some(&token),
            )
            .await;
        assert_eq!(r.status, 422, "{}: {}", t.label, r.body);
        let r = t
            .get("/api/v1/stock/racks/9999/stickers", Some(&token))
            .await;
        assert_eq!(r.status, 404, "{}: {}", t.label, r.body);

        let r = t
            .get("/api/v1/document-fields?kind=cell_sticker", Some(&token))
            .await;
        assert_eq!(r.status, 200);
        let keys: Vec<_> = r
            .body
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["placeholder"].as_str().unwrap())
            .collect();
        assert!(keys.contains(&"ячейка.код"), "{keys:?}");
        assert!(!keys.contains(&"заказ.номер"));

        let r = t
            .get("/api/v1/documents/cell_sticker?format=png", Some(&token))
            .await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        assert_eq!(r.body["pages"].as_array().unwrap().len(), 1);
    }
}
