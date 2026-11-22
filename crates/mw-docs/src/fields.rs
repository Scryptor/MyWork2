//! Substitution fields: `{заказ.номер}` in template text is the value `order.number` of the document data.

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

static FIELDS: [FieldDef; 33] = [
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
        "Устройство принято на диагностику. Срок диагностики — до 3 рабочих дней.",
        "",
    ),
    field(
        "firm.issue_terms",
        "фирма.договор_выдачи",
        "Условия выдачи (договор)",
        "Устройство получено, претензий к внешнему виду и работе нет.",
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
];

/// All substitution fields, in the order of the «insert field» menu.
pub fn fields() -> &'static [FieldDef] {
    &FIELDS
}

/// The field written in template text as `{name}`: case does not matter, «ё» = «е».
pub fn field_by_placeholder(name: &str) -> Option<&'static FieldDef> {
    let wanted = normalize_search(name);
    FIELDS
        .iter()
        .find(|f| normalize_search(f.placeholder) == wanted)
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
