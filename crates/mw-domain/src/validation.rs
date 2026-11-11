use serde::{Deserialize, Serialize};

/// Ошибка валидации поля: имя поля и машинный код (`required`, `invalid`, `too_short`,
/// `duplicate`, `out_of_range`, `estimate_range`, `deleted`, `last_admin`, `last_issued_status`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldError {
    pub field: String,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_error_required() {
        assert_eq!(
            FieldError::required("brand"),
            FieldError::new("brand", "required")
        );
    }
}
