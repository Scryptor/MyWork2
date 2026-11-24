//! Склад: позиции по HTTP — список, сводка, добавление, правка с версией, удаление, история.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::Method;
use common::{Listen, apis, ids};
use mw_domain::permission::Permission;
use serde_json::json;

#[tokio::test]
async fn stock_item_endpoints() {
    for t in apis(Listen::Network).await {
        let token = t
            .token_with(
                "sklad",
                &[
                    Permission::StockView,
                    Permission::StockCreate,
                    Permission::StockEdit,
                    Permission::StockDelete,
                ],
            )
            .await;
        let post = async |body: serde_json::Value| {
            t.send(Method::POST, "/api/v1/stock", Some(&token), Some(body))
                .await
        };
        // Поля, которых нет в теле, — пустые.
        let r = post(json!({
            "part_type": "Дисплей", "device_type": "Телефон", "model": "A52",
            "quantity": 1, "price": 250000, "restock_threshold": 2
        }))
        .await;
        assert_eq!(
            (
                r.status,
                &r.body["display_name"],
                &r.body["name"],
                &r.body["needs_restock"],
                &r.body["version"]
            ),
            (
                201,
                &json!("Дисплей Телефон A52"),
                &json!(""),
                &json!(true),
                &json!(1)
            ),
            "{}: {}",
            t.label,
            r.body
        );
        let display = r.body["id"].as_i64().unwrap();
        let r = post(json!({ "name": "Шлейф", "quantity": 9, "price": 10000 })).await;
        assert_eq!(r.status, 201, "{}: {}", t.label, r.body);
        let cable = r.body["id"].as_i64().unwrap();

        let r = post(json!({ "nam": "Опечатка" })).await;
        assert_eq!(
            (r.status, &r.body["code"]),
            (400, &json!("bad_request")),
            "{}",
            t.label
        );
        let r = post(json!({})).await;
        assert_eq!(
            (
                r.status,
                &r.body["fields"][0]["field"],
                &r.body["fields"][0]["code"]
            ),
            (422, &json!("name"), &json!("required")),
            "{}",
            t.label
        );

        let page = t
            .get(
                "/api/v1/stock?query=%D0%B4%D0%B8%D1%81%D0%BF%D0%BB%D0%B5%D0%B9&restock=true&sort=quantity&descending=true",
                Some(&token),
            )
            .await;
        assert_eq!(
            (page.status, ids(&page.body), &page.body["total"]),
            (200, vec![display], &json!(1)),
            "{}: {}",
            t.label,
            page.body
        );
        let page = t
            .get("/api/v1/stock?sort=quantity&descending=true", Some(&token))
            .await;
        assert_eq!(ids(&page.body), vec![cable, display], "{}", t.label);
        let page = t.get("/api/v1/stock", Some(&token)).await;
        assert_eq!(ids(&page.body), vec![display, cable], "{}", t.label);
        let page = t.get("/api/v1/stock?descending=true", Some(&token)).await;
        assert_eq!(ids(&page.body), vec![cable, display], "{}", t.label);
        let page = t.get("/api/v1/stock?limit=1&offset=1", Some(&token)).await;
        assert_eq!(
            (ids(&page.body), &page.body["total"]),
            (vec![cable], &json!(2)),
            "{}",
            t.label
        );
        assert_eq!(
            t.get("/api/v1/stock?page=2", Some(&token)).await.status,
            400,
            "{}",
            t.label
        );
        let r = t.get("/api/v1/stock/summary", Some(&token)).await;
        assert_eq!(
            (r.status, &r.body),
            (200, &json!({ "restock": 1 })),
            "{}",
            t.label
        );
        let r = t
            .get(&format!("/api/v1/stock/{display}"), Some(&token))
            .await;
        assert_eq!(
            (r.status, &r.body["price"], &r.body["part_type"]),
            (200, &json!(250000), &json!("Дисплей")),
            "{}",
            t.label
        );

        let patch = async |version: Option<i64>, body: serde_json::Value| {
            let path = match version {
                Some(v) => format!("/api/v1/stock/{display}?version={v}"),
                None => format!("/api/v1/stock/{display}"),
            };
            t.send(Method::PATCH, &path, Some(&token), Some(body)).await
        };
        let r = patch(None, json!({ "quantity": 5 })).await;
        assert_eq!(
            (r.status, &r.body["code"]),
            (400, &json!("bad_request")),
            "{}",
            t.label
        );
        let r = patch(Some(1), json!({ "quantity": 5 })).await;
        assert_eq!(
            (
                r.status,
                &r.body["quantity"],
                &r.body["version"],
                &r.body["needs_restock"]
            ),
            (200, &json!(5), &json!(2), &json!(false)),
            "{}: {}",
            t.label,
            r.body
        );
        let r = patch(Some(1), json!({ "quantity": 6 })).await;
        assert_eq!(
            (
                r.status,
                &r.body["code"],
                &r.body["current_version"],
                &r.body["message"]
            ),
            (
                409,
                &json!("conflict"),
                &json!(2),
                &json!(format!(
                    "Запись изменена другим пользователем: позиция склада № {display}. Обновите данные"
                ))
            ),
            "{}",
            t.label
        );
        assert_eq!(
            patch(Some(2), json!({ "quantity": null })).await.status,
            400,
            "{}",
            t.label
        );
        assert_eq!(
            patch(Some(2), json!({ "colour": "Чёрный" })).await.status,
            400,
            "{}",
            t.label
        );
        let r = patch(Some(2), json!({ "quantity": -1 })).await;
        assert_eq!(
            (r.status, &r.body["fields"][0]["field"]),
            (422, &json!("quantity")),
            "{}",
            t.label
        );

        let r = t
            .get(&format!("/api/v1/stock/{display}/history"), Some(&token))
            .await;
        let actions: Vec<&str> = r.body["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["action"].as_str().unwrap())
            .collect();
        assert_eq!(
            (r.status, actions, &r.body["items"][0]["stock_item_id"]),
            (200, vec!["stock.updated", "stock.created"], &json!(display)),
            "{}",
            t.label
        );
        let r = t
            .get(
                &format!("/api/v1/stock/{display}/history?limit=1&offset=1"),
                Some(&token),
            )
            .await;
        assert_eq!(
            (&r.body["items"][0]["action"], &r.body["total"]),
            (&json!("stock.created"), &json!(2)),
            "{}",
            t.label
        );

        let r = t
            .send(
                Method::DELETE,
                &format!("/api/v1/stock/{display}"),
                Some(&token),
                None,
            )
            .await;
        assert_eq!(r.status, 400, "{}: без версии", t.label);
        let r = t
            .send(
                Method::DELETE,
                &format!("/api/v1/stock/{display}?version=2"),
                Some(&token),
                None,
            )
            .await;
        assert_eq!(r.status, 204, "{}: {}", t.label, r.body);
        let r = t
            .get(&format!("/api/v1/stock/{display}"), Some(&token))
            .await;
        assert_eq!(
            (r.status, &r.body["code"], &r.body["message"]),
            (
                404,
                &json!("not_found"),
                &json!(format!("Не найдено: позиция склада № {display}"))
            ),
            "{}",
            t.label
        );
        assert_eq!(
            ids(&t.get("/api/v1/stock", Some(&token)).await.body),
            vec![cable],
            "{}",
            t.label
        );
        assert_eq!(
            t.get("/api/v1/stock/summary", Some(&token)).await.body,
            json!({ "restock": 0 }),
            "{}",
            t.label
        );
    }
}

#[tokio::test]
async fn stock_rights_and_item_journal() {
    for t in apis(Listen::Network).await {
        let admin = t.admin_token().await;
        let r = t
            .send(
                Method::POST,
                "/api/v1/stock",
                Some(&admin),
                Some(json!({ "name": "Дисплей" })),
            )
            .await;
        assert_eq!(r.status, 201, "{}: {}", t.label, r.body);
        let id = r.body["id"].as_i64().unwrap();

        let viewer = t.token_with("priemka", &[Permission::StockView]).await;
        let r = t
            .get(&format!("/api/v1/audit?stock_item_id={id}"), Some(&viewer))
            .await;
        assert_eq!(
            (r.status, &r.body["total"], &r.body["items"][0]["action"]),
            (200, &json!(1), &json!("stock.created")),
            "{}: {}",
            t.label,
            r.body
        );
        let r = t.get("/api/v1/audit", Some(&viewer)).await;
        assert_eq!(
            (r.status, &r.body["permission"]),
            (403, &json!("audit_view")),
            "{}",
            t.label
        );
        let r = t
            .send(
                Method::POST,
                "/api/v1/stock",
                Some(&viewer),
                Some(json!({ "name": "Шлейф" })),
            )
            .await;
        assert_eq!(
            (r.status, &r.body["permission"]),
            (403, &json!("stock_create")),
            "{}",
            t.label
        );

        let master = t.token_with("master", &[Permission::OrdersEdit]).await;
        for path in [
            "/api/v1/stock".to_owned(),
            "/api/v1/stock/summary".to_owned(),
            format!("/api/v1/stock/{id}"),
            format!("/api/v1/stock/{id}/history"),
            format!("/api/v1/audit?stock_item_id={id}"),
        ] {
            let r = t.get(&path, Some(&master)).await;
            assert_eq!(
                (r.status, &r.body["permission"]),
                (403, &json!("stock_view")),
                "{}: {path}",
                t.label
            );
        }
        let r = t
            .send(
                Method::PATCH,
                &format!("/api/v1/stock/{id}?version=1"),
                Some(&viewer),
                Some(json!({ "quantity": 2 })),
            )
            .await;
        assert_eq!(
            (r.status, &r.body["permission"]),
            (403, &json!("stock_edit")),
            "{}",
            t.label
        );
        let r = t
            .send(
                Method::DELETE,
                &format!("/api/v1/stock/{id}?version=1"),
                Some(&viewer),
                None,
            )
            .await;
        assert_eq!(
            (r.status, &r.body["permission"]),
            (403, &json!("stock_delete")),
            "{}",
            t.label
        );
    }
}

fn png() -> Vec<u8> {
    let img = image::RgbImage::from_pixel(40, 30, image::Rgb([200, 30, 30]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

#[tokio::test]
async fn stock_photo_endpoints() {
    use axum::body::Body;
    use axum::http::Request;

    for t in apis(Listen::Network).await {
        let token = t
            .token_with(
                "sklad",
                &[
                    Permission::StockView,
                    Permission::StockCreate,
                    Permission::StockEdit,
                ],
            )
            .await;
        let viewer = t.token_with("smotr", &[Permission::StockView]).await;
        let r = t
            .send(
                Method::POST,
                "/api/v1/stock",
                Some(&token),
                Some(json!({ "name": "Дисплей" })),
            )
            .await;
        let id = r.body["id"].as_i64().unwrap();
        let post = async |ct: &str, tok: &str| {
            let req = Request::post(format!("/api/v1/stock/{id}/photos?name=a.png"))
                .header("host", "127.0.0.1:8080")
                .header("content-type", ct)
                .header("authorization", format!("Bearer {tok}"))
                .body(Body::from(png()))
                .unwrap();
            t.request(req).await
        };
        let r = post("multipart/form-data; boundary=x", &token).await;
        assert_eq!(r.status, 415, "{}", t.label);
        let r = post("application/octet-stream", &viewer).await;
        assert_eq!(r.status, 403, "{}", t.label);
        let r = post("application/octet-stream", &token).await;
        assert_eq!(
            (r.status, &r.body["stock_item_id"]),
            (201, &json!(id)),
            "{}",
            t.label
        );
        let aid = r.body["id"].as_i64().unwrap();
        let r = t
            .get(&format!("/api/v1/stock/{id}/photos"), Some(&viewer))
            .await;
        assert_eq!((r.status, r.body.as_array().map(Vec::len)), (200, Some(1)));
        let r = t
            .get_raw(&format!("/api/v1/attachments/{aid}/thumb"), &viewer)
            .await;
        assert_eq!(r.0, 200, "{}", t.label);
        let nobody = t.token_with("nikto", &[Permission::OrdersEdit]).await;
        let r = t
            .get_raw(&format!("/api/v1/attachments/{aid}/file"), &nobody)
            .await;
        assert_eq!(r.0, 403, "{}", t.label);
    }
}
