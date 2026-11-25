//! Ссылки фирмы для QR-кодов: встроенные и свои, правила списка.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::search::normalize_search;
use crate::validation::FieldError;

/// Встроенные ссылки по порядку: `id` и название. Их можно выключить и переименовать, но не удалить.
pub const BUILTIN_LINKS: [(&str, &str); 4] = [
    ("yandex_maps", "Яндекс Карты"),
    ("2gis", "2ГИС"),
    ("telegram", "Telegram"),
    ("max", "MAX"),
];
/// Сколько ссылок в списке не больше.
pub const MAX_LINKS: usize = 20;
/// Длина названия, знаков.
pub const LINK_NAME_MAX: usize = 60;
/// Длина адреса, знаков.
pub const LINK_URL_MAX: usize = 500;
/// Длина `id`, знаков.
pub const LINK_ID_MAX: usize = 40;

/// Ссылка фирмы. Тот же тип — во входе и в выходе.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct FirmLink {
    /// Стабильный код `[a-z0-9_]`, 1–40 знаков; от названия не зависит.
    pub id: String,
    /// Название для людей, 1–60 знаков, без учёта регистра не повторяется.
    pub name: String,
    /// Адрес: пусто или `http://`/`https://`, до 500 знаков.
    pub url: String,
    /// Включена ли ссылка: выключенная печатается пустой.
    pub enabled: bool,
}

/// Формат `id` ссылки: `[a-z0-9_]`, 1–40 знаков.
pub fn is_link_id(id: &str) -> bool {
    (1..=LINK_ID_MAX).contains(&id.chars().count())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Ссылка: `http://` или `https://` (любой регистр) и после них хоть что-то, без пробельных и
/// управляющих знаков.
pub fn is_http_link(url: &str) -> bool {
    let lower = url.to_lowercase();
    let rest = lower
        .strip_prefix("http://")
        .or_else(|| lower.strip_prefix("https://"));
    rest.is_some_and(|rest| !rest.is_empty())
        && !url.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// Список по умолчанию: четыре встроенные ссылки, включены, адреса пусты.
pub fn default_links() -> Vec<FirmLink> {
    BUILTIN_LINKS
        .iter()
        .map(|(id, name)| FirmLink {
            id: (*id).to_owned(),
            name: (*name).to_owned(),
            url: String::new(),
            enabled: true,
        })
        .collect()
}

/// Обрезает пробелы в названиях и адресах и проверяет список. Ошибки — с путями `firm_links` и
/// `firm_links.{i}.{поле}`.
pub fn normalize_links(mut list: Vec<FirmLink>) -> Result<Vec<FirmLink>, Vec<FieldError>> {
    let mut errors = Vec::new();
    if list.len() > MAX_LINKS {
        errors.push(FieldError::new("firm_links", "too_many"));
    }
    if BUILTIN_LINKS
        .iter()
        .any(|(id, _)| !list.iter().any(|l| l.id == *id))
    {
        errors.push(FieldError::new("firm_links", "builtin_missing"));
    }
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    for (i, link) in list.iter_mut().enumerate() {
        link.name = link.name.trim().to_owned();
        link.url = link.url.trim().to_owned();
        let at = |field: &str| format!("firm_links.{i}.{field}");
        if !is_link_id(&link.id) {
            errors.push(FieldError::new(at("id"), "invalid"));
        } else if !ids.insert(link.id.clone()) {
            errors.push(FieldError::new(at("id"), "duplicate"));
        }
        if link.name.is_empty() {
            errors.push(FieldError::required(at("name")));
        } else if link.name.chars().count() > LINK_NAME_MAX {
            errors.push(FieldError::new(at("name"), "too_long"));
        } else if !names.insert(normalize_search(&link.name)) {
            errors.push(FieldError::new(at("name"), "duplicate"));
        }
        if !link.url.is_empty() {
            if link.url.chars().count() > LINK_URL_MAX {
                errors.push(FieldError::new(at("url"), "too_long"));
            } else if !is_http_link(&link.url) {
                errors.push(FieldError::new(at("url"), "not_a_link"));
            }
        }
    }
    if errors.is_empty() {
        Ok(list)
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn link(id: &str, name: &str, url: &str) -> FirmLink {
        FirmLink {
            id: id.to_owned(),
            name: name.to_owned(),
            url: url.to_owned(),
            enabled: true,
        }
    }

    fn with(extra: FirmLink) -> Vec<FirmLink> {
        let mut l = default_links();
        l.push(extra);
        l
    }

    fn errors(list: Vec<FirmLink>) -> Vec<(String, String)> {
        normalize_links(list)
            .unwrap_err()
            .into_iter()
            .map(|e| (e.field, e.code))
            .collect()
    }

    fn one(path: &str, code: &str) -> Vec<(String, String)> {
        vec![(path.to_owned(), code.to_owned())]
    }

    #[test]
    fn default_links_are_the_four_builtins_enabled_and_empty() {
        let l = default_links();
        assert_eq!(
            l.iter()
                .map(|x| (x.id.as_str(), x.name.as_str()))
                .collect::<Vec<_>>(),
            BUILTIN_LINKS.to_vec()
        );
        assert!(l.iter().all(|x| x.enabled && x.url.is_empty()));
    }

    #[test]
    fn normalize_trims_and_accepts_own_links() {
        let out = normalize_links(with(link(
            "link_k3x9",
            "  ВКонтакте ",
            " https://vk.com/x ",
        )))
        .unwrap();
        assert_eq!(out.len(), 5);
        assert_eq!(out[4].name, "ВКонтакте");
        assert_eq!(out[4].url, "https://vk.com/x");
        assert_eq!(out[4].id, "link_k3x9");
    }

    #[test]
    fn normalize_rejects_bad_lists() {
        // Повтор имени без учёта регистра: считается у более поздней ссылки.
        assert_eq!(
            errors(with(link("link_a", "telegram", ""))),
            one("firm_links.4.name", "duplicate")
        );
        // Нет встроенной ссылки.
        let mut no_max = default_links();
        no_max.pop();
        assert_eq!(errors(no_max), one("firm_links", "builtin_missing"));
        // Неверный и повторный id.
        assert_eq!(
            errors(with(link("Bad-Id", "Свой", ""))),
            one("firm_links.4.id", "invalid")
        );
        assert_eq!(
            errors(with(link("telegram", "Свой", ""))),
            one("firm_links.4.id", "duplicate")
        );
        assert_eq!(
            errors(with(link(&"a".repeat(41), "Свой", ""))),
            one("firm_links.4.id", "invalid")
        );
        // Имя.
        assert_eq!(
            errors(with(link("link_a", "  ", ""))),
            one("firm_links.4.name", "required")
        );
        assert_eq!(
            errors(with(link("link_a", &"я".repeat(61), ""))),
            one("firm_links.4.name", "too_long")
        );
        // Адрес.
        assert_eq!(
            errors(with(link("link_a", "Свой", "ftp://x"))),
            one("firm_links.4.url", "not_a_link")
        );
        assert_eq!(
            errors(with(link(
                "link_a",
                "Свой",
                &format!("https://{}", "x".repeat(493))
            ))),
            one("firm_links.4.url", "too_long")
        );
        // Слишком много.
        let mut many = default_links();
        for i in 0..17 {
            many.push(link(&format!("link_{i}"), &format!("Свой {i}"), ""));
        }
        assert_eq!(many.len(), 21);
        assert_eq!(errors(many), one("firm_links", "too_many"));
    }

    #[test]
    fn exactly_twenty_links_and_http_in_any_case_are_fine() {
        let mut list = default_links();
        for i in 0..16 {
            list.push(link(&format!("link_{i}"), &format!("Свой {i}"), ""));
        }
        list[2].url = "HTTPS://t.me/x".to_owned();
        assert_eq!(normalize_links(list).unwrap().len(), 20);
    }
}
