//! Извлекатели, которые отвечают ошибками в едином формате.

use axum::extract::{FromRequest, FromRequestParts};
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use crate::error::ApiError;

/// Тело запроса и ответа в JSON.
#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(ApiError))]
pub(crate) struct Json<T>(pub(crate) T);

impl<T: Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}

/// Параметры пути.
#[allow(dead_code)] // используется обработчиками следующих задач
#[derive(FromRequestParts)]
#[from_request(via(axum::extract::Path), rejection(ApiError))]
pub(crate) struct Path<T>(pub(crate) T);

/// Параметры запроса; повторяющиеся параметры собираются в `Vec`.
#[allow(dead_code)] // используется обработчиками следующих задач
#[derive(FromRequestParts)]
#[from_request(via(axum_extra::extract::Query), rejection(ApiError))]
pub(crate) struct Query<T>(pub(crate) T);

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::{get, post};
    use http_body_util::BodyExt;
    use serde::Deserialize;
    use tower::ServiceExt;

    #[derive(Deserialize)]
    struct Ids {
        #[serde(default)]
        id: Vec<i64>,
    }

    #[derive(Deserialize)]
    struct BodyDto {
        #[allow(dead_code)]
        name: String,
    }

    fn app() -> Router {
        Router::new()
            .route(
                "/q",
                get(|Query(q): Query<Ids>| async move { format!("{:?}", q.id) }),
            )
            .route(
                "/p/{n}",
                get(|Path(n): Path<i64>| async move { n.to_string() }),
            )
            .route("/j", post(|Json(_): Json<BodyDto>| async { "ok" }))
    }

    async fn call(req: Request<Body>) -> (u16, serde_json::Value) {
        let resp = app().oneshot(req).await.unwrap();
        let status = resp.status().as_u16();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or_else(|_| {
                serde_json::Value::String(String::from_utf8_lossy(&bytes).into())
            }),
        )
    }

    #[tokio::test]
    async fn query_collects_repeated_parameters() {
        let (status, body) = call(Request::get("/q?id=1&id=2").body(Body::empty()).unwrap()).await;
        assert_eq!((status, body), (200, serde_json::json!([1, 2])));
    }

    #[tokio::test]
    async fn rejections_become_json_bad_requests() {
        let (status, body) = call(Request::get("/p/abc").body(Body::empty()).unwrap()).await;
        assert_eq!(
            (status, &body["code"]),
            (400, &serde_json::json!("bad_request"))
        );
        assert!(
            body["message"]
                .as_str()
                .unwrap()
                .starts_with("Некорректный запрос: ")
        );
        let bad_type = Request::post("/j")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"name": 5}"#))
            .unwrap();
        let (status, body) = call(bad_type).await;
        assert_eq!(
            (status, &body["code"]),
            (400, &serde_json::json!("bad_request"))
        );
    }
}
