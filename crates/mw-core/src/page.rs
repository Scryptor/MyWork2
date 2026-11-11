use mw_domain::validation::FieldError;

use crate::CoreError;

/// Страница результатов и общее число подходящих записей.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: u64,
}

/// Допустимая страница: размер 1..=500, смещение не больше `i64::MAX` (иначе БД отвергнет запрос).
pub(crate) fn validate_page(limit: u64, offset: u64) -> Result<(), CoreError> {
    let mut errors = Vec::new();
    if !(1..=500).contains(&limit) {
        errors.push(FieldError::new("limit", "out_of_range"));
    }
    if offset > i64::MAX as u64 {
        errors.push(FieldError::new("offset", "out_of_range"));
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(CoreError::Validation(errors))
    }
}
