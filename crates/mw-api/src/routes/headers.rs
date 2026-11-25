//! Заголовки ответов с файлами: имя вложения и запрет кэша.

use axum::body::Body;
use axum::http::{HeaderValue, header};
use axum::response::Response;
use mw_core::ExportFile;

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

/// Значение `Content-Disposition` с именем по RFC 5987: `filename*=UTF-8''…` (любые символы) и,
/// если дано, запасное ASCII-имя в `filename="…"` для программ без поддержки `filename*`.
/// `ascii` не должно содержать кавычек и `;`: его составляют вызывающие из букв и точки.
pub(super) fn content_disposition(kind: Disposition, name: &str, ascii: Option<&str>) -> String {
    let kind = match kind {
        Disposition::Inline => "inline",
        Disposition::Attachment => "attachment",
    };
    let encoded = percent_encode(name);
    match ascii {
        Some(ascii) => format!("{kind}; filename=\"{ascii}\"; filename*=UTF-8''{encoded}"),
        None => format!("{kind}; filename*=UTF-8''{encoded}"),
    }
}

/// `Content-Disposition: attachment` с русским именем и запасным ASCII-именем.
pub(super) fn attachment(name: &str, ascii: &str) -> String {
    content_disposition(Disposition::Attachment, name, Some(ascii))
}

/// Процентное кодирование для `filename*` (RFC 5987): всё, кроме `A-Za-z0-9-._~`.
pub(super) fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Готовый файл выгрузки: тип, длина, имя для сохранения; не кэшируется.
pub(super) fn export_response(file: ExportFile) -> Response {
    let len = file.bytes.len();
    let mut response = Response::new(Body::from(file.bytes));
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(file.content_type),
    );
    headers.insert(header::CONTENT_LENGTH, HeaderValue::from(len));
    if let Ok(v) = HeaderValue::from_str(&attachment(&file.name, file.ascii_name)) {
        headers.insert(header::CONTENT_DISPOSITION, v);
    }
    no_store(response)
}

/// Ответ с данными клиента или меняющийся под тем же адресом не оседает в кэше браузера и прокси.
pub(super) fn no_store(mut response: Response) -> Response {
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_percent_encoded() {
        assert_eq!(percent_encode("акт.pdf"), "%D0%B0%D0%BA%D1%82.pdf");
        assert_eq!(percent_encode("a b\"c'.png"), "a%20b%22c%27.png");
    }

    #[test]
    fn attachment_has_a_fallback_and_an_encoded_name() {
        assert_eq!(
            attachment("Отчёт.xlsx", "report.xlsx"),
            "attachment; filename=\"report.xlsx\"; filename*=UTF-8''%D0%9E%D1%82%D1%87%D1%91%D1%82.xlsx"
        );
        assert_eq!(
            content_disposition(Disposition::Inline, "a b.png", None),
            "inline; filename*=UTF-8''a%20b.png"
        );
    }
}
