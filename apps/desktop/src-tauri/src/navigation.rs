//! Куда может перейти окно и что оно может сохранить. Окно показывает только свой сервер: его права
//! (`capabilities`) не достаются чужим страницам. Звонок, письмо и внешние сайты открывает система.

use url::{Origin, Url};

/// Что делать с переходом окна.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Navigation {
    /// Страница своего сервера.
    Allow,
    /// Отдать системе: `tel:` (звонок), `mailto:` (письмо), внешний сайт — в браузере.
    System,
    /// Не открывать.
    Block,
}

/// Решение о переходе на `to` для окна, открытого на источнике `own`.
pub fn navigation(to: &Url, own: &Origin) -> Navigation {
    if to.origin() == *own || to.as_str() == "about:blank" {
        return Navigation::Allow;
    }
    match to.scheme() {
        "tel" | "mailto" | "http" | "https" => Navigation::System,
        _ => Navigation::Block,
    }
}

/// Загрузка разрешена только со своего сервера (в том числе `blob:` своего источника).
pub fn download_allowed(url: &Url, own: &Origin) -> bool {
    // У `blob:` источник — тот, что внутри: `blob:http://127.0.0.1:<порт>/…` — наш.
    url.origin() == *own
}

#[cfg(test)]
mod tests {
    use super::*;

    fn own() -> Origin {
        Url::parse("http://127.0.0.1:50946/").unwrap().origin()
    }

    fn go(to: &str) -> Navigation {
        navigation(&Url::parse(to).unwrap(), &own())
    }

    fn save(url: &str) -> bool {
        download_allowed(&Url::parse(url).unwrap(), &own())
    }

    #[test]
    fn pages_of_the_own_server_open_in_the_window() {
        assert_eq!(go("http://127.0.0.1:50946/"), Navigation::Allow);
        assert_eq!(go("http://127.0.0.1:50946/orders?q=1#x"), Navigation::Allow);
        assert_eq!(
            go("blob:http://127.0.0.1:50946/6f1c0d5e-0000-4000-8000-000000000000"),
            Navigation::Allow
        );
        // Пустая страница ничего не показывает и прав не получает (`remote.urls` её не пускают).
        assert_eq!(go("about:blank"), Navigation::Allow);
    }

    #[test]
    fn a_call_a_letter_and_outside_sites_go_to_the_system() {
        assert_eq!(go("tel:+79001234567"), Navigation::System);
        assert_eq!(go("mailto:client@example.com"), Navigation::System);
        assert_eq!(go("https://example.com/"), Navigation::System);
        assert_eq!(go("http://example.com/"), Navigation::System);
        // Другой порт — другой источник: это уже не наш сервер.
        assert_eq!(go("http://127.0.0.1:8080/"), Navigation::System);
        assert_eq!(go("http://localhost:50946/"), Navigation::System);
    }

    #[test]
    fn everything_else_is_blocked() {
        assert_eq!(go("file:///etc/passwd"), Navigation::Block);
        assert_eq!(go("javascript:alert(1)"), Navigation::Block);
        assert_eq!(go("data:text/html,<h1>x</h1>"), Navigation::Block);
        assert_eq!(go("ftp://example.com/"), Navigation::Block);
        assert_eq!(go("smb://server/share"), Navigation::Block);
        assert_eq!(go("about:srcdoc"), Navigation::Block);
        assert_eq!(go("blob:https://example.com/uuid"), Navigation::Block);
    }

    #[test]
    fn only_files_of_the_own_server_are_saved() {
        assert!(save(
            "blob:http://127.0.0.1:50946/6f1c0d5e-0000-4000-8000-000000000000"
        ));
        assert!(save("http://127.0.0.1:50946/api/v1/attachments/12/file"));
        assert!(!save("blob:http://127.0.0.1:8080/uuid"));
        assert!(!save("https://example.com/file.exe"));
        assert!(!save("data:text/plain,hello"));
    }
}
