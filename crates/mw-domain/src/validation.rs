use serde::{Deserialize, Serialize};

/// Ошибка валидации поля: имя поля и машинный код (`required`, `invalid`, `too_short`,
/// `duplicate`, `out_of_range`, `estimate_range`, `deleted`, `last_admin`, `last_issued_status`,
/// `comma`, `changes_term`, `stale`, `closed`, `too_many`, `too_long`, `too_large`, `no_room`, `text_budget`, `too_many_pages`, `too_many_pixels`, `requires_a4`,
/// `not_allowed`, `nested_columns`, `fill_in_columns`, `unsupported`, `mismatch`, `not_editable`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct FieldError {
    /// Имя поля запроса.
    pub field: String,
    /// Машинный код ошибки.
    pub code: String,
    /// Особое сообщение для человека вместо общего по коду; в API не передаётся отдельным полем.
    #[serde(skip)]
    #[ts(skip)]
    #[schema(ignore)]
    pub message: Option<&'static str>,
}

impl FieldError {
    pub fn new(field: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            code: code.into(),
            message: None,
        }
    }

    /// Ошибка с собственным сообщением (для кода, который в разных местах значит разное).
    pub fn with_message(
        field: impl Into<String>,
        code: impl Into<String>,
        message: &'static str,
    ) -> Self {
        Self {
            message: Some(message),
            ..Self::new(field, code)
        }
    }

    pub fn required(field: impl Into<String>) -> Self {
        Self::new(field, "required")
    }

    /// Сообщение для человека по коду ошибки; неизвестный код — общее сообщение.
    pub fn message_ru(&self) -> &'static str {
        if let Some(m) = self.message {
            return m;
        }
        if self.field == "logo" && self.code == "unsupported" {
            return "Логотип — картинка PNG, JPEG или WebP";
        }
        if self.field == "order_ids" && self.code == "too_many" {
            return "Не больше 100 заказов за раз";
        }
        match self.code.as_str() {
            "required" => "Заполните поле",
            "too_short" => "Слишком короткое значение",
            "duplicate" => "Такое значение уже есть",
            "out_of_range" => "Значение вне допустимого диапазона",
            "estimate_range" => "Нижняя граница больше верхней",
            "deleted" => "Запись удалена",
            "last_admin" => "Нельзя оставить программу без администратора",
            "last_issued_status" => "Нужен хотя бы один статус вида «Выдан»",
            "line_break" => "Введите одно значение в одну строку",
            "comma" => "В списке запятая разделяет значения: введите одно значение без запятой",
            "stale" => "Порядок тем временем изменили: обновите список и повторите",
            "changes_term" => {
                "Срок гарантии в заказах изменился бы: замените значение только в справочнике"
            }
            "not_empty" => "База не пуста",
            "replace_unsupported" => "Замена возможна только для базы SQLite",
            "too_many" => "Слишком много элементов",
            "too_long" => "Слишком длинный текст",
            "too_large" => "Файл слишком большой",
            "no_room" => "Поля страницы не оставляют места для содержимого",
            "text_budget" => {
                "В шаблоне слишком много текста: сократите тексты (не больше 20 000 знаков)"
            }
            "too_many_pages" => {
                "Документ получается слишком большим (больше 50 страниц на заказ или 200 в пачке): уберите лишнее или увеличьте страницу"
            }
            "too_many_pixels" => "Страница слишком велика для такого разрешения: уменьшите dpi",
            "requires_a4" => {
                "Раскладка «2 на лист A4» — только для страницы A4 в книжной ориентации"
            }
            "not_allowed" => "Не подходит для этого документа",
            "nested_columns" => "Колонки нельзя вкладывать в колонки",
            "fill_in_columns" => "Растягивать штрихкод на высоту нельзя внутри колонок",
            "unsupported" => "Файл сделан в более новой версии программы",
            "mismatch" => "Шаблон другого документа",
            "not_editable" => "Этот документ не настраивается",
            _ => "Недопустимое значение",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_codes_have_russian_messages() {
        assert_eq!(
            FieldError::new("database", "not_empty").message_ru(),
            "База не пуста"
        );
        assert_eq!(
            FieldError::new("mode", "replace_unsupported").message_ru(),
            "Замена возможна только для базы SQLite"
        );
    }

    #[test]
    fn logo_unsupported_has_its_own_message() {
        assert_eq!(
            FieldError::new("logo", "unsupported").message_ru(),
            "Логотип — картинка PNG, JPEG или WebP"
        );
        assert_ne!(
            FieldError::new("logo", "unsupported").message_ru(),
            FieldError::new("file", "unsupported").message_ru()
        );
    }

    #[test]
    fn field_error_required() {
        assert_eq!(
            FieldError::required("brand"),
            FieldError::new("brand", "required")
        );
    }
}
