//! Раздача собранного интерфейса: файлы из папки, неизвестный путь вне `/api/` и `/_app/` —
//! `index.html`.
//!
//! Файлы `/_app/immutable/**` названы по хешу содержимого и кэшируются навсегда; остальное
//! (в том числе `index.html`) не кэшируется. Отсутствующий файл `/_app/...` — 404, а не
//! страница приложения: скрипт, пришедший как HTML, ломает интерфейс непонятной ошибкой.

use std::convert::Infallible;
use std::path::Path;

use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderValue, Method, header};
use axum::response::{IntoResponse, Response};
use tower::ServiceExt;
use tower_http::services::{ServeDir, ServeFile};

use crate::error::ApiError;

const IMMUTABLE_PREFIX: &str = "/_app/immutable/";
const IMMUTABLE_CACHE: &str = "public, max-age=31536000, immutable";

#[derive(Clone)]
struct Files {
    /// Папка с запасным `index.html` для маршрутов приложения.
    spa: ServeDir<ServeFile>,
    /// Та же папка без запасного пути: для `/_app/...`.
    assets: ServeDir,
}

/// Обработчик для всего, что не нашлось среди маршрутов API.
pub(crate) fn handler(
    dir: &Path,
) -> impl Fn(Request) -> std::pin::Pin<Box<dyn Future<Output = Response> + Send>> + Clone + Send + 'static
{
    let files = Files {
        spa: ServeDir::new(dir).fallback(ServeFile::new(dir.join("index.html"))),
        assets: ServeDir::new(dir),
    };
    move |req: Request| {
        let files = files.clone();
        Box::pin(async move { serve(files, req).await })
    }
}

async fn serve(files: Files, req: Request) -> Response {
    let path = req.uri().path().to_owned();
    if path == "/api" || path.starts_with("/api/") {
        return ApiError::path_not_found().into_response();
    }
    if !matches!(*req.method(), Method::GET | Method::HEAD) {
        return ApiError::method_not_allowed().into_response();
    }
    let in_app = path == "/_app" || path.starts_with("/_app/");
    let req = req.map(Body::new);
    let mut resp = if in_app {
        let result: Result<_, Infallible> = files.assets.oneshot(req).await;
        match result {
            Ok(resp) => resp.map(Body::new),
            Err(never) => match never {},
        }
    } else {
        let result: Result<_, Infallible> = files.spa.oneshot(req).await;
        match result {
            Ok(resp) => resp.map(Body::new),
            Err(never) => match never {},
        }
    };
    if path.starts_with(IMMUTABLE_PREFIX) && resp.status().is_success() {
        resp.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static(IMMUTABLE_CACHE),
        );
    }
    resp
}
