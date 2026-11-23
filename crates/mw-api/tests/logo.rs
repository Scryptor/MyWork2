//! Логотип фирмы: загрузка телом octet-stream, отдача, пределы.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::body::Body;
use axum::http::Request;
use common::{Listen, TestApi, TestResponse, apis};
use mw_domain::permission::Permission;

const OCTET: &str = "application/octet-stream";
const PATH: &str = "/api/v1/settings/logo";

fn png() -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(40, 30, image::Rgba([200, 30, 30, 255]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

async fn put(t: &TestApi, token: &str, content_type: &str, body: Vec<u8>) -> TestResponse {
    let req = Request::put(PATH)
        .header("host", "127.0.0.1:8080")
        .header("content-type", content_type)
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap();
    t.request(req).await
}

#[tokio::test]
async fn upload_serve_and_delete() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        assert_eq!(t.get(PATH, Some(&token)).await.status, 404, "{}", t.label);
        assert_eq!(
            t.get(&format!("{PATH}/info"), Some(&token)).await.status,
            404
        );
        let r = put(&t, &token, OCTET, png()).await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        assert_eq!(
            (r.body["width"].as_i64(), r.body["height"].as_i64()),
            (Some(40), Some(30))
        );
        let info = t.get(&format!("{PATH}/info"), Some(&token)).await;
        assert_eq!(info.status, 200);
        assert_eq!(info.body, r.body);
        let (status, headers, bytes) = t.get_raw(PATH, &token).await;
        assert_eq!(status, 200);
        assert_eq!(headers["content-type"], "image/png");
        assert_eq!(headers["cache-control"], "private, no-store");
        assert_eq!(
            image::guess_format(&bytes).unwrap(),
            image::ImageFormat::Png
        );
        let r = t
            .send(axum::http::Method::DELETE, PATH, Some(&token), None)
            .await;
        assert_eq!(r.status, 204, "{}", t.label);
        assert_eq!(t.get(PATH, Some(&token)).await.status, 404);
    }
}

#[tokio::test]
async fn upload_content_types_limits_and_rights() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        for ct in [
            "multipart/form-data; boundary=x",
            "text/plain",
            "application/x-www-form-urlencoded",
        ] {
            let r = put(&t, &token, ct, png()).await;
            assert_eq!(r.status, 415, "{} {ct}", t.label);
        }
        let r = put(&t, &token, OCTET, b"hello".to_vec()).await;
        assert_eq!(r.status, 422, "{}", t.label);
        assert_eq!(r.body["fields"][0]["field"], "logo");
        assert_eq!(
            r.body["fields"][0]["message"],
            "Логотип — картинка PNG, JPEG или WebP"
        );
        let r = put(&t, &token, OCTET, vec![0; 5 * 1024 * 1024 + 1]).await;
        assert_eq!(r.status, 422, "{}", t.label);
        assert_eq!(r.body["fields"][0]["code"], "too_large");
        let r = put(&t, &token, OCTET, vec![0; 7 * 1024 * 1024]).await;
        assert_eq!(r.status, 413, "{}", t.label);

        let weak = t.token_with("weak", &[Permission::OrdersEdit]).await;
        assert_eq!(put(&t, &weak, OCTET, png()).await.status, 403);
        let r = t
            .send(axum::http::Method::DELETE, PATH, Some(&weak), None)
            .await;
        assert_eq!(r.status, 403);
    }
}
