//! Защитные слои: журнал запросов, ограничение времени, проверка `Host`,
//! `Content-Type` изменяющих методов, заголовки безопасности.

use std::time::{Duration, Instant};

use axum::Router;
use axum::extract::Request;
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, header};
use axum::middleware::{Next, from_fn};
use axum::response::{IntoResponse, Response};

use crate::error::ApiError;

/// Журнал запроса: метод, путь без строки запроса, статус, длительность.
pub(crate) async fn request_log(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_owned();
    let started = Instant::now();
    let response = next.run(req).await;
    tracing::info!(
        method = %method,
        path = %path,
        status = response.status().as_u16(),
        ms = started.elapsed().as_millis() as u64,
        "запрос"
    );
    response
}

/// Ограничивает время обработки: дольше `limit` — 503 `unavailable`. Кроме `POST UNTIMED_LEGACY_CHECK`.
pub(crate) fn with_timeout<S: Clone + Send + Sync + 'static>(
    router: Router<S>,
    limit: Duration,
) -> Router<S> {
    router.layer(from_fn(move |req: Request, next: Next| async move {
        if req.method() == Method::POST && req.uri().path() == UNTIMED_LEGACY_CHECK {
            return next.run(req).await;
        }
        match tokio::time::timeout(limit, next.run(req)).await {
            Ok(response) => response,
            Err(_) => ApiError::timed_out().into_response(),
        }
    }))
}

/// Проверка папки MyWork 2 без ограничения времени: она читает и разбирает всю старую базу и проверяет
/// каждый файл, на большой или медленной папке это дольше `REQUEST_TIMEOUT`. Маршрут только для сервера на
/// loopback и для того, у кого есть права импорта, и ничего не пишет.
pub(crate) const UNTIMED_LEGACY_CHECK: &str = "/api/v1/legacy-import/check";

/// Имя хоста без порта и квадратных скобок, в нижнем регистре.
fn host_name(raw: &str) -> String {
    let name = match raw.strip_prefix('[') {
        Some(rest) => rest.split(']').next().unwrap_or(""),
        None => raw.split(':').next().unwrap_or(""),
    };
    name.to_ascii_lowercase()
}

/// Защита от DNS rebinding: на loopback-сервере годятся только локальные имена.
pub(crate) async fn require_local_host(req: Request, next: Next) -> Response {
    let raw = req
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
        .or_else(|| req.uri().authority().map(|a| a.as_str().to_owned()));
    let allowed = raw
        .map(|r| matches!(host_name(&r).as_str(), "127.0.0.1" | "localhost" | "::1"))
        .unwrap_or(false);
    if allowed {
        next.run(req).await
    } else {
        ApiError::host_forbidden().into_response()
    }
}

/// Маршруты загрузки вложений (`POST /orders/{id}/attachments`,
/// `POST /intake-drafts/{key}/attachments`): только они принимают `application/octet-stream`.
fn is_upload_route(req: &Request) -> bool {
    let logo = req.method() == Method::PUT && req.uri().path() == "/api/v1/settings/logo";
    if req.method() != Method::POST && !logo {
        return false;
    }
    let octet = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|essence| {
            essence
                .trim()
                .eq_ignore_ascii_case("application/octet-stream")
        });
    if !octet {
        return false;
    }
    if logo {
        return true;
    }
    let path = req.uri().path();
    ["/api/v1/orders/", "/api/v1/intake-drafts/"]
        .iter()
        .filter_map(|prefix| path.strip_prefix(prefix))
        .filter_map(|rest| rest.strip_suffix("/attachments"))
        .any(|id| !id.is_empty() && !id.contains('/'))
}

/// Изменяющие методы принимаются только с `Content-Type: application/json`:
/// чужая страница в браузере без разрешения CORS такой запрос не отправит.
pub(crate) async fn require_json_content_type(req: Request, next: Next) -> Response {
    let changing = matches!(
        *req.method(),
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    );
    if changing {
        let is_json = req
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(';').next())
            .is_some_and(|essence| essence.trim().eq_ignore_ascii_case("application/json"));
        if !is_json && !is_upload_route(&req) {
            return ApiError::json_required().into_response();
        }
    }
    next.run(req).await
}

/// Ставит `Cache-Control: no-store`, `X-Content-Type-Options: nosniff` и запрет показа
/// во фрейме чужого сайта (`X-Frame-Options` и `frame-ancestors` в `Content-Security-Policy`).
pub(crate) fn set_security_headers(headers: &mut HeaderMap) {
    // Отдача файлов задаёт свои `Cache-Control` и `Content-Security-Policy`: их не затираем.
    headers
        .entry(header::CACHE_CONTROL)
        .or_insert(HeaderValue::from_static("no-store"));
    headers.insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers
        .entry(header::CONTENT_SECURITY_POLICY)
        .or_insert(HeaderValue::from_static("frame-ancestors 'none'"));
}

/// Заголовки безопасности у каждого ответа, прошедшего через слой.
/// Ответы-отказы внешних слоёв получают их в `ApiError::into_response`.
pub(crate) async fn security_headers(req: Request, next: Next) -> Response {
    let mut response = next.run(req).await;
    set_security_headers(response.headers_mut());
    response
}

/// Ответ на неизвестный путь.
pub(crate) async fn not_found_fallback() -> ApiError {
    ApiError::path_not_found()
}

/// Ответ на неподдерживаемый метод.
pub(crate) async fn method_not_allowed_fallback() -> ApiError {
    ApiError::method_not_allowed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::routing::get;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn slow_requests_answer_503_unavailable() {
        let app = with_timeout(
            Router::new().route(
                "/",
                get(|| async {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    "поздно"
                }),
            ),
            Duration::from_millis(20),
        );
        let resp = app
            .oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 503);
        assert_eq!(resp.headers()["cache-control"], "no-store");
        assert_eq!(resp.headers()["x-content-type-options"], "nosniff");
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["code"], "unavailable");
    }

    #[tokio::test]
    async fn checking_a_legacy_folder_is_not_limited_in_time() {
        let slow = || async {
            tokio::time::sleep(Duration::from_millis(100)).await;
            "готово"
        };
        let app = with_timeout(
            Router::new()
                .route(UNTIMED_LEGACY_CHECK, axum::routing::post(slow).get(slow))
                .route("/api/v1/legacy-import", axum::routing::post(slow)),
            Duration::from_millis(20),
        );
        let status = |method: Method, path: &'static str| {
            let app = app.clone();
            async move {
                let request = Request::builder()
                    .method(method)
                    .uri(path)
                    .body(Body::empty())
                    .unwrap();
                app.oneshot(request).await.unwrap().status().as_u16()
            }
        };
        assert_eq!(status(Method::POST, UNTIMED_LEGACY_CHECK).await, 200);
        // Только этот маршрут и только POST: остальное ограничено, как раньше.
        assert_eq!(status(Method::GET, UNTIMED_LEGACY_CHECK).await, 503);
        assert_eq!(status(Method::POST, "/api/v1/legacy-import").await, 503);
    }

    #[test]
    fn the_untimed_path_is_the_route_of_the_legacy_folder_check() {
        let api = crate::openapi();
        let item = api
            .paths
            .paths
            .get(UNTIMED_LEGACY_CHECK)
            .expect("маршрут проверки папки");
        assert!(item.post.is_some());
    }

    #[test]
    fn host_names_are_normalized() {
        assert_eq!(host_name("LOCALHOST:1"), "localhost");
        assert_eq!(host_name("[::1]:8080"), "::1");
        assert_eq!(host_name("127.0.0.1"), "127.0.0.1");
    }
}
