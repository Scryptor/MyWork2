use serde::{Deserialize, Serialize};

/// Ошибка валидации поля: имя поля и машинный код (`required`, `invalid`, `too_short`,
/// `duplicate`, `out_of_range`, `estimate_range`, `deleted`, `last_admin`, `last_issued_status`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct FieldError {
    /// Имя поля запроса.
    pub field: String,
    /// Машинный код ошибки.
    pub code: String,
}

impl FieldError {
    pub fn new(field: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            code: code.into(),
        }
    }

    pub fn required(field: impl Into<String>) -> Self {
        Self::new(field, "required")
    }

    /// Сообщение для человека по коду ошибки; неизвестный код — общее сообщение.
    pub fn message_ru(&self) -> &'static str {
        match self.code.as_str() {
            "required" => "Заполните поле",
            "too_short" => "Слишком короткое значение",
            "duplicate" => "Такое значение уже есть",
            "out_of_range" => "Значение вне допустимого диапазона",
            "estimate_range" => "Нижняя граница больше верхней",
            "deleted" => "Запись удалена",
            "last_admin" => "Нельзя оставить программу без администратора",
            "last_issued_status" => "Нужен хотя бы один статус вида «Выдан»",
            "not_empty" => "База не пуста",
            "replace_unsupported" => "Замена возможна только для базы SQLite",
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
    fn field_error_required() {
        assert_eq!(
            FieldError::required("brand"),
            FieldError::new("brand", "required")
        );
    }
}
