//! Substitution fields: `{заказ.номер}` in template text is the value `order.number` of the document data.

use mw_domain::documents::DocumentKind;
use mw_domain::links::is_link_id;
use mw_domain::search::normalize_search;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldDef {
    /// Key in `DocumentData::values`.
    pub key: &'static str,
    /// Name in template text, written in braces: `{заказ.номер}`.
    pub placeholder: &'static str,
    /// Name for people (the «insert field» menu).
    pub label: &'static str,
    /// Value in sample documents.
    pub sample: &'static str,
    /// Printed instead of an empty value (a blank form draws a line instead); `""` — nothing.
    pub empty: &'static str,
}

const fn field(
    key: &'static str,
    placeholder: &'static str,
    label: &'static str,
    sample: &'static str,
    empty: &'static str,
) -> FieldDef {
    FieldDef {
        key,
        placeholder,
        label,
        sample,
        empty,
    }
}

const NOT_SET: &str = "Не указан";

/// The key of a firm link in `DocumentData::values`: `link.<id>`.
pub const LINK_KEY_PREFIX: &str = "link.";
/// The name of a firm link in template text: `{ссылка.<id>}`.
pub const LINK_PLACEHOLDER_PREFIX: &str = "ссылка.";

/// A link of the firm that older templates write under its former name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyLink {
    /// The former key in `DocumentData::values`; it is still filled for the author's code.
    pub key: &'static str,
    /// The former name in template text.
    pub placeholder: &'static str,
    /// The `id` of the link it now stands for.
    pub id: &'static str,
}

/// The three links that were fields before the list of links.
pub const LEGACY_LINKS: [LegacyLink; 3] = [
    LegacyLink {
        key: "firm.telegram",
        placeholder: "фирма.телеграм",
        id: "telegram",
    },
    LegacyLink {
        key: "firm.google_maps",
        placeholder: "фирма.гугл_карты",
        id: "google_maps",
    },
    LegacyLink {
        key: "firm.yandex_maps",
        placeholder: "фирма.яндекс_карты",
        id: "yandex_maps",
    },
];

/// Sample addresses of the links by `id`, for sample documents.
pub const LINK_SAMPLES: [(&str, &str); 5] = [
    ("yandex_maps", "https://yandex.ru/maps/?text=example"),
    ("2gis", "https://2gis.ru/example"),
    ("telegram", "https://t.me/example_service"),
    ("max", "https://max.ru/example_service"),
    ("google_maps", "https://maps.google.com/?q=example"),
];

static FIELDS: [FieldDef; 39] = [
    field("order.number", "заказ.номер", "Номер заказа", "1234", ""),
    field(
        "order.barcode",
        "заказ.штрихкод",
        "Штрихкод заказа",
        "200000012342",
        "",
    ),
    field(
        "order.received",
        "заказ.дата_приёма",
        "Дата приёма",
        "22 ноября 2026",
        "",
    ),
    field(
        "order.issued",
        "заказ.дата_выдачи",
        "Дата выдачи",
        "25 ноября 2026",
        "",
    ),
    field(
        "order.device",
        "заказ.устройство",
        "Устройство (тип, бренд и модель)",
        "Смартфон Samsung Galaxy A52",
        "",
    ),
    field(
        "order.device_type",
        "заказ.тип_устройства",
        "Тип устройства",
        "Смартфон",
        "",
    ),
    field("order.brand", "заказ.бренд", "Бренд", "Samsung", ""),
    field("order.model", "заказ.модель", "Модель", "Galaxy A52", ""),
    field(
        "order.serial",
        "заказ.серийный_номер",
        "Серийный номер",
        "R58R00000AB",
        NOT_SET,
    ),
    // Пароль и ключ стандартные акты не печатают (акт отдают клиенту): мастерская ставит их в свой.
    field(
        "order.password",
        "заказ.пароль",
        "Пароль устройства",
        "1234",
        "",
    ),
    field(
        "order.pattern",
        "заказ.графический_ключ",
        "Графический ключ",
        "1 · 5 · 8 · 9 · 6 · 3",
        "",
    ),
    field("order.color", "заказ.цвет", "Цвет", "Чёрный", ""),
    field(
        "order.completeness",
        "заказ.комплектность",
        "Комплектность",
        "Чехол, зарядное устройство",
        "",
    ),
    field(
        "order.condition",
        "заказ.состояние",
        "Внешний вид",
        "Царапины на корпусе",
        "",
    ),
    field(
        "order.malfunction",
        "заказ.неисправность",
        "Неисправность",
        "Не включается",
        "",
    ),
    field(
        "order.work_done",
        "заказ.работы",
        "Выполненные работы",
        "Замена разъёма питания",
        "",
    ),
    field("order.master", "заказ.мастер", "Мастер", "Иванов И. И.", ""),
    field(
        "order.estimate",
        "заказ.оценка",
        "Предварительная стоимость",
        "от 1 000 до 2 500 руб.",
        "Не оговорена",
    ),
    field(
        "order.prepayment",
        "заказ.предоплата",
        "Предоплата",
        "500 руб.",
        "",
    ),
    field("order.discount", "заказ.скидка", "Скидка", "200 руб.", ""),
    field(
        "order.price",
        "заказ.стоимость",
        "Стоимость",
        "2 000 руб.",
        "",
    ),
    field("order.due", "заказ.к_оплате", "К оплате", "1 300 руб.", ""),
    field(
        "order.warranty",
        "заказ.гарантия",
        "Гарантия",
        "30 дней",
        "",
    ),
    field(
        "client.name",
        "клиент.фио",
        "ФИО клиента",
        "Петров Пётр Петрович",
        "",
    ),
    field(
        "client.phone",
        "клиент.телефон",
        "Телефон клиента",
        "+7 900 000-00-00",
        "",
    ),
    field(
        "client.phones",
        "клиент.телефоны",
        "Все телефоны клиента",
        "+7 900 000-00-00, 20-00-00",
        "",
    ),
    field(
        "client.address",
        "клиент.адрес",
        "Адрес клиента",
        "г. Энск, ул. Примерная, д. 1",
        NOT_SET,
    ),
    field(
        "firm.name",
        "фирма.название",
        "Название фирмы",
        "Сервисный центр «Пример»",
        "",
    ),
    field(
        "firm.phone",
        "фирма.телефон",
        "Телефон фирмы",
        "+7 900 000-00-01",
        "",
    ),
    field(
        "firm.details",
        "фирма.сведения",
        "Сведения о фирме (адрес, часы работы)",
        "г. Энск, ул. Мастеров, д. 5\nЕжедневно 10:00–20:00",
        "",
    ),
    field(
        "firm.legal",
        "фирма.реквизиты",
        "Реквизиты фирмы",
        "ИП Примеров П. П., ИНН 000000000000",
        "",
    ),
    field(
        "firm.intake_terms",
        "фирма.договор_приёма",
        "Условия приёма (договор)",
        "Устройство принято на диагностику без вскрытия и проверки внутренних повреждений.\n\
         Срок диагностики — до 3 рабочих дней, срок ремонта — до 45 дней со дня приёма.\n\
         Сервис не отвечает за данные в памяти устройства: сохраните их заранее.",
        "",
    ),
    field(
        "firm.issue_terms",
        "фирма.договор_выдачи",
        "Условия выдачи (договор)",
        "Устройство получено, претензий к внешнему виду и работе нет.\n\
         Гарантия распространяется на выполненные работы и установленные запчасти.",
        "",
    ),
    field(
        "point.name",
        "точка.название",
        "Точка приёма",
        "Центральная",
        "",
    ),
    field(
        "doc.date",
        "документ.дата",
        "Дата документа",
        "22 ноября 2026",
        "",
    ),
    field("cell.code", "ячейка.код", "Код ячейки", "B-2-4", ""),
    field("cell.rack", "ячейка.шкаф", "Шкаф (подпись)", "Дисплеи", ""),
    field("cell.shelf", "ячейка.полка", "Полка", "2", ""),
    field("cell.number", "ячейка.номер", "Номер ячейки", "4", ""),
];

/// All substitution fields, in the order of the «insert field» menu.
pub fn fields() -> &'static [FieldDef] {
    &FIELDS
}

/// The fields offered for `kind`: a cell sticker has the cell's, the firm's and the date; every other
/// kind has everything but the cell's.
pub fn fields_for(kind: DocumentKind) -> impl Iterator<Item = &'static FieldDef> {
    FIELDS.iter().filter(move |f| {
        let cell = f.key.starts_with("cell.");
        if kind == DocumentKind::CellSticker {
            cell || f.key.starts_with("firm.") || f.key == "doc.date"
        } else {
            !cell
        }
    })
}

/// The field written in template text as `{name}`: case does not matter, «ё» = «е».
pub fn field_by_placeholder(name: &str) -> Option<&'static FieldDef> {
    let wanted = normalize_search(name);
    FIELDS
        .iter()
        .find(|f| normalize_search(f.placeholder) == wanted)
}

/// The key (`link.<id>`) of the link written in template text as `{name}`: `{ссылка.<id>}`, or the
/// former name of one of the three old links. Case does not matter, «ё» = «е». Any well-formed `id`
/// is a link, whether the firm has it or not: a link that is missing or off is just empty.
pub fn link_by_placeholder(name: &str) -> Option<String> {
    let wanted = normalize_search(name);
    if let Some(legacy) = LEGACY_LINKS
        .iter()
        .find(|l| normalize_search(l.placeholder) == wanted)
    {
        return Some(format!("{LINK_KEY_PREFIX}{}", legacy.id));
    }
    let id = wanted.strip_prefix(&normalize_search(LINK_PLACEHOLDER_PREFIX))?;
    is_link_id(id).then(|| format!("{LINK_KEY_PREFIX}{id}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font_covers;

    #[test]
    fn keys_and_placeholders_are_unique() {
        let mut keys: Vec<_> = FIELDS.iter().map(|f| f.key).collect();
        let mut names: Vec<_> = FIELDS
            .iter()
            .map(|f| normalize_search(f.placeholder))
            .collect();
        keys.sort_unstable();
        keys.dedup();
        names.sort_unstable();
        names.dedup();
        assert_eq!(keys.len(), FIELDS.len());
        assert_eq!(names.len(), FIELDS.len());
    }

    #[test]
    fn placeholders_are_found_ignoring_case_and_yo() {
        let key = |name| field_by_placeholder(name).map(|f| f.key);
        assert_eq!(key("заказ.номер"), Some("order.number"));
        assert_eq!(key("ЗАКАЗ.Номер"), Some("order.number"));
        assert_eq!(key("заказ.дата_приема"), Some("order.received"));
        assert_eq!(key("ЗАКАЗ.ДАТА_ПРИЁМА"), Some("order.received"));
        assert_eq!(key("заказ.нет_такого"), None);
        assert_eq!(key("order.number"), None);
    }

    #[test]
    fn link_fields_are_not_static() {
        assert_eq!(FIELDS.len(), 39);
        for key in ["firm.telegram", "firm.google_maps", "firm.yandex_maps"] {
            assert!(FIELDS.iter().all(|f| f.key != key), "{key}");
        }
        assert_eq!(field_by_placeholder("фирма.телеграм").map(|f| f.key), None);
    }

    #[test]
    fn cell_fields_belong_to_the_cell_sticker() {
        let names = |kind| fields_for(kind).map(|f| f.placeholder).collect::<Vec<_>>();
        let cell = names(DocumentKind::CellSticker);
        assert!(cell.contains(&"ячейка.код"));
        assert!(cell.contains(&"фирма.название"));
        assert!(!cell.contains(&"заказ.номер"));
        for kind in DocumentKind::ALL {
            if kind != DocumentKind::CellSticker {
                let other = names(kind);
                assert!(other.iter().all(|n| !n.starts_with("ячейка.")), "{kind:?}");
                assert!(other.contains(&"заказ.номер"), "{kind:?}");
            }
        }
        assert_eq!(
            field_by_placeholder("Ячейка.Шкаф").map(|f| f.key),
            Some("cell.rack")
        );
    }

    #[test]
    fn link_placeholders_resolve() {
        let key = |name| link_by_placeholder(name);
        assert_eq!(key("ссылка.telegram").as_deref(), Some("link.telegram"));
        assert_eq!(key("Ссылка.2gis").as_deref(), Some("link.2gis"));
        assert_eq!(key("фирма.телеграм").as_deref(), Some("link.telegram"));
        assert_eq!(key("Фирма.Гугл_Карты").as_deref(), Some("link.google_maps"));
        assert_eq!(
            key("фирма.яндекс_карты").as_deref(),
            Some("link.yandex_maps")
        );
        assert_eq!(key("ссылка.Bad-Id"), None);
        assert_eq!(key("ссылка."), None);
        assert_eq!(key("ссылка"), None);
        assert_eq!(key("заказ.номер"), None);
        // A well-formed id of no link is still a link.
        assert_eq!(key("ссылка.link_gone").as_deref(), Some("link.link_gone"));
    }

    #[test]
    fn link_samples_are_example_addresses_in_the_font() {
        let ids: Vec<_> = LINK_SAMPLES.iter().map(|(id, _)| *id).collect();
        assert_eq!(
            ids,
            ["yandex_maps", "2gis", "telegram", "max", "google_maps"]
        );
        for (id, url) in LINK_SAMPLES {
            assert!(mw_domain::links::is_link_id(id));
            assert!(mw_domain::links::is_http_link(url), "{url}");
            assert!(url.contains("example"), "{url}");
        }
        for legacy in LEGACY_LINKS {
            assert!(LINK_SAMPLES.iter().any(|(id, _)| *id == legacy.id));
            assert_eq!(legacy.key, format!("firm.{}", legacy.id));
        }
    }

    /// Пароль и графический ключ — сразу после серийного номера; пустые не печатаются ничем.
    #[test]
    fn password_and_pattern_follow_the_serial() {
        let at = FIELDS.iter().position(|f| f.key == "order.serial").unwrap();
        let next: Vec<_> = FIELDS[at + 1..at + 3]
            .iter()
            .map(|f| (f.key, f.placeholder, f.label, f.sample, f.empty))
            .collect();
        assert_eq!(
            next,
            [
                (
                    "order.password",
                    "заказ.пароль",
                    "Пароль устройства",
                    "1234",
                    ""
                ),
                (
                    "order.pattern",
                    "заказ.графический_ключ",
                    "Графический ключ",
                    "1 · 5 · 8 · 9 · 6 · 3",
                    ""
                ),
            ]
        );
        let key = |name| field_by_placeholder(name).map(|f| f.key);
        assert_eq!(key("Заказ.Пароль"), Some("order.password"));
        assert_eq!(key("заказ.графический_ключ"), Some("order.pattern"));
    }

    #[test]
    fn empty_texts_match_mywork2() {
        let empty = |key| FIELDS.iter().find(|f| f.key == key).map(|f| f.empty);
        assert_eq!(empty("order.serial"), Some("Не указан"));
        assert_eq!(empty("client.address"), Some("Не указан"));
        assert_eq!(empty("order.estimate"), Some("Не оговорена"));
        assert_eq!(empty("order.master"), Some(""));
    }

    #[test]
    fn samples_are_printable() {
        let sample = |key| FIELDS.iter().find(|f| f.key == key).map(|f| f.sample);
        let barcode = sample("order.barcode").unwrap_or_default();
        assert!(mw_domain::barcode::is_valid_upc_a(barcode));
        assert_eq!(
            mw_domain::barcode::order_barcode(1234).as_deref(),
            Some(barcode)
        );
        for f in &FIELDS {
            let all = format!("{}{}{}", f.label, f.sample, f.empty);
            assert!(font_covers(&all).is_empty(), "{}: {:?}", f.key, all);
            assert!(!f.sample.is_empty(), "{}", f.key);
        }
    }
}
