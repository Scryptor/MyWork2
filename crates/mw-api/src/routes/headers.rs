//! Заголовки ответов с файлами: имя вложения и запрет кэша.

use axum::http::{HeaderValue, header};
use axum::response::Response;

/// Как показывать файл: в окне или сохранить.
#[derive(Debug, Clone, Copy)]
pub(super) enum Disposition {
    Inline,
    Attachment,
}

/// Ставит `Content-Disposition` с именем файла; имя только из ASCII без кавычек (так его
/// составляют вызывающие), иначе заголовок не ставится.
pub(super) fn with_disposition(
    mut response: Response,
    disposition: Disposition,
    name: &str,
) -> Response {
    let kind = match disposition {
        Disposition::Inline => "inline",
        Disposition::Attachment => "attachment",
    };
    if let Ok(v) = HeaderValue::from_str(&format!("{kind}; filename=\"{name}\"")) {
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, v);
    }
    response
}

/// Ответ с данными клиента или меняющийся под тем же адресом не оседает в кэше браузера и прокси.
pub(super) fn no_store(mut response: Response) -> Response {
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    response
}
