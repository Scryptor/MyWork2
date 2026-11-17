//! Раздача собранного интерфейса тем же сервером, что и API.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Method, Request};
use chrono::{TimeZone, Utc};
use http_body_util::BodyExt;
use mw_api::{ApiConfig, router};
use mw_core::{Core, FixedClock};
use tower::ServiceExt;

const INDEX: &str = "<!doctype html><title>mw</title>";
const SCRIPT: &str = "console.log('x');";

struct Site {
    router: Router,
    _dir: std::path::PathBuf,
}

async fn site(with_ui: bool, addr: SocketAddr) -> Site {
    let root = std::env::temp_dir().join(format!(
        "mw-static-ui-{}-{}",
        std::process::id(),
        rand_suffix()
    ));
    let ui = root.join("ui");
    std::fs::create_dir_all(ui.join("_app")).unwrap();
    std::fs::write(ui.join("index.html"), INDEX).unwrap();
    std::fs::write(ui.join("_app/x.js"), SCRIPT).unwrap();
    std::fs::write(root.join("secret"), "секрет").unwrap();
    let (_, db) = mw_db::testing::all_backends().await.remove(0);
    let clock = Arc::new(FixedClock::new(
        Utc.with_ymd_and_hms(2026, 11, 11, 9, 0, 0).unwrap(),
    ));
    let core = Core::open(db, clock, "UTC").await.unwrap();
    let config = ApiConfig::for_listen_addr(addr).with_ui_dir(with_ui.then_some(ui));
    Site {
        router: router(core, config),
        _dir: root,
    }
}

fn rand_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

async fn call(
    site: &Site,
    method: Method,
    path: &str,
    host: &str,
) -> (u16, axum::http::HeaderMap, String) {
    let mut req = Request::builder()
        .method(method)
        .uri(path)
        .header("host", host)
        .header("content-type", "application/json")
        .body(Body::empty())
        .unwrap();
    req.extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([10, 0, 0, 1], 50000))));
    let resp = site.router.clone().oneshot(req).await.unwrap();
    let status = resp.status().as_u16();
    let headers = resp.headers().clone();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        headers,
        String::from_utf8_lossy(&bytes).into_owned(),
    )
}

fn net() -> SocketAddr {
    ([0, 0, 0, 0], 8080).into()
}

#[tokio::test]
async fn serves_files_and_falls_back_to_index() {
    let s = site(true, net()).await;
    let (code, headers, body) = call(&s, Method::GET, "/_app/x.js", "h").await;
    assert_eq!((code, body.as_str()), (200, SCRIPT));
    assert_eq!(headers["x-content-type-options"], "nosniff");
    assert_eq!(headers["cache-control"], "no-store");
    let (code, _, body) = call(&s, Method::GET, "/orders/10481", "h").await;
    assert_eq!((code, body.as_str()), (200, INDEX));
    let (code, _, body) = call(&s, Method::GET, "/", "h").await;
    assert_eq!((code, body.as_str()), (200, INDEX));
    let (code, _, body) = call(&s, Method::HEAD, "/_app/x.js", "h").await;
    assert_eq!((code, body.as_str()), (200, ""));
    let (code, _, body) = call(&s, Method::GET, "/api/v1/nope", "h").await;
    assert_eq!(code, 404);
    assert!(body.contains("\"not_found\""), "{body}");
    let (code, _, body) = call(&s, Method::GET, "/api", "h").await;
    assert_eq!(code, 404);
    assert!(body.contains("\"not_found\""), "{body}");
    let (code, _, body) = call(&s, Method::GET, "/api/v1/server-info", "h").await;
    assert_eq!(code, 200);
    assert!(body.contains("\"server_time\""), "{body}");
    let (code, _, body) = call(&s, Method::POST, "/orders/1", "h").await;
    assert_eq!(code, 405);
    assert!(body.contains("\"bad_request\""), "{body}");
}

#[tokio::test]
async fn does_not_leave_the_folder() {
    let s = site(true, net()).await;
    for path in [
        "/../secret",
        "/%2e%2e/secret",
        "/_app/../../secret",
        "/..%2fsecret",
    ] {
        let (code, _, body) = call(&s, Method::GET, path, "h").await;
        assert!(!body.contains("секрет"), "{path}: {body}");
        assert!(code == 200 || code == 404 || code == 400, "{path}: {code}");
    }
}

#[tokio::test]
async fn without_a_folder_unknown_paths_are_json_404() {
    let s = site(false, net()).await;
    let (code, _, body) = call(&s, Method::GET, "/", "h").await;
    assert_eq!(code, 404);
    assert!(body.contains("\"not_found\""), "{body}");
}

#[tokio::test]
async fn loopback_host_check_applies_to_files() {
    let s = site(true, ([127, 0, 0, 1], 8080).into()).await;
    let (code, _, _) = call(&s, Method::GET, "/", "evil.example").await;
    assert_eq!(code, 403);
    let (code, _, body) = call(&s, Method::GET, "/", "localhost:8080").await;
    assert_eq!((code, body.as_str()), (200, INDEX));
}

#[tokio::test]
async fn no_response_can_be_framed_by_another_site() {
    let s = site(true, ([127, 0, 0, 1], 8080).into()).await;
    for (path, host) in [
        ("/_app/x.js", "localhost:8080"),
        ("/orders/10481", "localhost:8080"),
        ("/api/v1/server-info", "localhost:8080"),
        ("/api/v1/nope", "localhost:8080"),
        ("/", "evil.example"),
    ] {
        let (_, headers, _) = call(&s, Method::GET, path, host).await;
        assert_eq!(headers["x-frame-options"], "DENY", "{path} {host}");
        assert_eq!(
            headers["content-security-policy"], "frame-ancestors 'none'",
            "{path} {host}"
        );
    }
}
