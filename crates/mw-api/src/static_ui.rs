//! Раздача собранного интерфейса: файлы из папки, неизвестный путь вне `/api/` — `index.html`.

use std::convert::Infallible;
use std::path::Path;

use axum::body::Body;
use axum::extract::Request;
use axum::http::Method;
use axum::response::{IntoResponse, Response};
use tower::ServiceExt;
use tower_http::services::{ServeDir, ServeFile};

use crate::error::ApiError;

/// Обработчик для всего, что не нашлось среди маршрутов API.
pub(crate) fn handler(
    dir: &Path,
) -> impl Fn(Request) -> std::pin::Pin<Box<dyn Future<Output = Response> + Send>> + Clone + Send + 'static
{
    let files = ServeDir::new(dir).fallback(ServeFile::new(dir.join("index.html")));
    move |req: Request| {
        let files = files.clone();
        Box::pin(async move { serve(files, req).await })
    }
}

async fn serve(files: ServeDir<ServeFile>, req: Request) -> Response {
    let path = req.uri().path();
    if path == "/api" || path.starts_with("/api/") {
        return ApiError::path_not_found().into_response();
    }
    if !matches!(*req.method(), Method::GET | Method::HEAD) {
        return ApiError::method_not_allowed().into_response();
    }
    let result: Result<_, Infallible> = files.oneshot(req.map(Body::new)).await;
    match result {
        Ok(resp) => resp.map(Body::new),
        Err(never) => match never {},
    }
}
