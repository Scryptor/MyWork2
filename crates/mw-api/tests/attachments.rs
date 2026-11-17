//! Вложения: загрузка, отдача файлов, подписи, удаление, черновики приёма.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::body::Body;
use axum::http::Request;
use common::{Listen, TestApi, TestResponse, apis, create_order, new_order};
use mw_domain::permission::Permission;
use serde_json::json;

fn png() -> Vec<u8> {
    let img = image::RgbImage::from_pixel(40, 30, image::Rgb([200, 30, 30]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

async fn upload(
    t: &TestApi,
    path: &str,
    token: &str,
    content_type: &str,
    body: Vec<u8>,
) -> TestResponse {
    let req = Request::post(path)
        .header("host", "127.0.0.1:8080")
        .header("content-type", content_type)
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap();
    t.request(req).await
}

const OCTET: &str = "application/octet-stream";

#[tokio::test]
async fn photo_upload_and_thumb() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        let o = create_order(&t, &token, new_order("Иван", "", "A", "B")).await;
        let id = o["id"].as_i64().unwrap();
        let r = upload(
            &t,
            &format!("/api/v1/orders/{id}/attachments?name=photo.png"),
            &token,
            OCTET,
            png(),
        )
        .await;
        assert_eq!(r.status, 201, "{}: {}", t.label, r.body);
        assert_eq!(r.body["kind"], "photo");
        let aid = r.body["id"].as_i64().unwrap();
        let thumb = t
            .get_raw(&format!("/api/v1/attachments/{aid}/thumb"), &token)
            .await;
        assert_eq!(thumb.0, 200);
        assert_eq!(thumb.1["content-type"], "image/jpeg");
        assert_eq!(
            thumb.1["cache-control"],
            "private, max-age=31536000, immutable"
        );
        assert_eq!(
            thumb.1["content-security-policy"],
            "sandbox; default-src 'none'; frame-ancestors 'none'"
        );
        assert_eq!(thumb.1["x-content-type-options"], "nosniff");
        assert!(
            thumb.1["content-disposition"]
                .to_str()
                .unwrap()
                .starts_with("inline")
        );
        assert_eq!(&thumb.2[..2], &[0xff, 0xd8]);
        let list = t
            .get(&format!("/api/v1/orders/{id}/attachments"), Some(&token))
            .await;
        assert_eq!(list.body.as_array().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn upload_content_types_and_limits() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        let o = create_order(&t, &token, new_order("Иван", "", "A", "B")).await;
        let id = o["id"].as_i64().unwrap();
        let path = format!("/api/v1/orders/{id}/attachments?name=a.png");
        for ct in [
            "multipart/form-data; boundary=x",
            "text/plain",
            "application/x-www-form-urlencoded",
        ] {
            let r = upload(&t, &path, &token, ct, png()).await;
            assert_eq!(r.status, 415, "{} {ct}", t.label);
        }
        // Octet-stream допустим только для загрузок.
        let r = upload(&t, "/api/v1/orders", &token, OCTET, b"{}".to_vec()).await;
        assert_eq!(r.status, 415);
        // Без имени.
        let r = upload(
            &t,
            &format!("/api/v1/orders/{id}/attachments"),
            &token,
            OCTET,
            png(),
        )
        .await;
        assert_eq!(r.status, 400);
        let r = upload(
            &t,
            &format!("/api/v1/orders/{id}/attachments?name=%20"),
            &token,
            OCTET,
            png(),
        )
        .await;
        assert_eq!(r.status, 400);
        // 26 МБ.
        let big = vec![0u8; 26 * 1024 * 1024];
        let r = upload(&t, &path, &token, OCTET, big).await;
        assert!(
            r.status == 413 || (r.status == 422 && r.body["code"] == "too_large"),
            "{}: {} {}",
            t.label,
            r.status,
            r.body
        );
        assert!(r.body["code"].is_string());
        // Обычные маршруты по-прежнему ограничены 1 МБ.
        let mut huge = new_order("x", "", "A", "B");
        huge["malfunction"] = json!("я".repeat(1_100_000));
        let r = t
            .send(
                axum::http::Method::POST,
                "/api/v1/orders",
                Some(&token),
                Some(huge),
            )
            .await;
        assert_eq!(r.status, 413);
    }
}

#[tokio::test]
async fn pdf_is_served_as_attachment_with_utf8_name() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        let o = create_order(&t, &token, new_order("Иван", "", "A", "B")).await;
        let id = o["id"].as_i64().unwrap();
        let r = upload(
            &t,
            &format!("/api/v1/orders/{id}/attachments?name=%D0%B0%D0%BA%D1%82.pdf"),
            &token,
            OCTET,
            b"%PDF-1.4 test".to_vec(),
        )
        .await;
        assert_eq!(r.status, 201, "{}: {}", t.label, r.body);
        let aid = r.body["id"].as_i64().unwrap();
        let f = t
            .get_raw(&format!("/api/v1/attachments/{aid}/file"), &token)
            .await;
        assert_eq!(f.0, 200);
        assert_eq!(
            f.1["content-disposition"],
            "attachment; filename*=UTF-8''%D0%B0%D0%BA%D1%82.pdf"
        );
        assert_eq!(f.2, b"%PDF-1.4 test");
        assert_eq!(f.1["cache-control"], "private, max-age=31536000, immutable");
    }
}

#[tokio::test]
async fn caption_delete_and_permissions() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        let o = create_order(&t, &token, new_order("Иван", "", "A", "B")).await;
        let id = o["id"].as_i64().unwrap();
        let r = upload(
            &t,
            &format!("/api/v1/orders/{id}/attachments?name=a.png"),
            &token,
            OCTET,
            png(),
        )
        .await;
        let aid = r.body["id"].as_i64().unwrap();
        let patch_path = format!("/api/v1/attachments/{aid}");
        let p = |body| {
            t.send(
                axum::http::Method::PATCH,
                &patch_path,
                Some(&token),
                Some(body),
            )
        };
        assert_eq!(
            p(json!({ "caption": "Царапина", "extra": 1 })).await.status,
            400
        );
        let r = p(json!({ "caption": "Царапина" })).await;
        assert_eq!((r.status, &r.body["caption"]), (200, &json!("Царапина")));

        let viewer = t.token_with("viewer", &[]).await;
        let r = upload(
            &t,
            &format!("/api/v1/orders/{id}/attachments?name=b.png"),
            &viewer,
            OCTET,
            png(),
        )
        .await;
        assert_eq!(r.status, 403);
        assert_eq!(
            t.get(&format!("/api/v1/orders/{id}/attachments"), Some(&viewer))
                .await
                .status,
            200
        );
        let d = t
            .send(
                axum::http::Method::DELETE,
                &format!("/api/v1/attachments/{aid}"),
                Some(&token),
                None,
            )
            .await;
        assert_eq!(d.status, 204);
        assert_eq!(
            t.get(&format!("/api/v1/attachments/{aid}/file"), Some(&token))
                .await
                .status,
            404
        );
    }
}

#[tokio::test]
async fn draft_flow_and_idempotent_intake() {
    for t in apis(Listen::Network).await {
        let token = t.token_with("clerk", &[Permission::OrdersCreate]).await;
        let key = "0f8fad5b-d9cb-469f-a165-70867728950e";
        for n in ["a.png", "b.png"] {
            let r = upload(
                &t,
                &format!("/api/v1/intake-drafts/{key}/attachments?name={n}"),
                &token,
                OCTET,
                png(),
            )
            .await;
            assert_eq!(r.status, 201, "{}: {}", t.label, r.body);
        }
        let l = t
            .get(
                &format!("/api/v1/intake-drafts/{key}/attachments"),
                Some(&token),
            )
            .await;
        assert_eq!(l.body.as_array().unwrap().len(), 2);
        // Выброшенный черновик.
        let other = "11111111-d9cb-469f-a165-70867728950e";
        upload(
            &t,
            &format!("/api/v1/intake-drafts/{other}/attachments?name=a.png"),
            &token,
            OCTET,
            png(),
        )
        .await;
        let d = t
            .send(
                axum::http::Method::DELETE,
                &format!("/api/v1/intake-drafts/{other}"),
                Some(&token),
                None,
            )
            .await;
        assert_eq!(d.status, 204);
        assert!(
            t.get(
                &format!("/api/v1/intake-drafts/{other}/attachments"),
                Some(&token)
            )
            .await
            .body
            .as_array()
            .unwrap()
            .is_empty()
        );

        let mut body = new_order("Иван", "", "A", "B");
        body["attachment_draft"] = json!(key);
        body["request_key"] = json!("req-1");
        let o = create_order(&t, &token, body.clone()).await;
        let id = o["id"].as_i64().unwrap();
        let in_order = t
            .get(&format!("/api/v1/orders/{id}/attachments"), Some(&token))
            .await;
        assert_eq!(in_order.body.as_array().unwrap().len(), 2, "{}", t.label);
        let again = create_order(&t, &token, body).await;
        assert_eq!(again["id"], o["id"]);
    }
}
