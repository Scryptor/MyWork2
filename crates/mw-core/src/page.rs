use mw_domain::validation::FieldError;

use crate::CoreError;

/// Страница результатов и общее число подходящих записей.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: u64,
}

/// Допустимый размер страницы: 1..=500.
#[allow(dead_code)] // используется списками из следующих задач
pub(crate) fn validate_limit(limit: u64) -> Result<(), CoreError> {
    if (1..=500).contains(&limit) {
        Ok(())
    } else {
        Err(CoreError::Validation(vec![FieldError::new(
            "limit",
            "out_of_range",
        )]))
    }
}
