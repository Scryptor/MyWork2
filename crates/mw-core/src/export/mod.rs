//! Выгрузки в файлы: общие типы и ограничения; таблицы Excel строит [`xlsx`].

use crate::CoreError;
use mw_domain::validation::FieldError;

pub(crate) mod clients;
pub(crate) mod contacts;
pub(crate) mod orders;
pub(crate) mod report;
pub(crate) mod xlsx;

/// Не больше строк в одном файле выгрузки; больше — ошибка `rows`/`too_many_rows`.
pub const EXPORT_ROWS_MAX: usize = 50_000;

/// Сколько файлов строится одновременно: каждая книга целиком лежит в памяти.
pub(crate) const MAX_CONCURRENT_EXPORTS: usize = 2;

/// Тип файла Excel (`.xlsx`).
pub(crate) const XLSX_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";

/// Готовый файл выгрузки.
#[derive(Debug, Clone)]
pub struct ExportFile {
    /// Имя для человека, например «Отчёт по заказам за всё время.xlsx».
    pub name: String,
    /// Запасное имя из латиницы для программ без поддержки `filename*`.
    pub ascii_name: &'static str,
    pub content_type: &'static str,
    pub bytes: Vec<u8>,
}

/// Дата в именах файлов: «25.11.2026».
pub(crate) fn dmy(date: chrono::NaiveDate) -> String {
    date.format("%d.%m.%Y").to_string()
}

/// Отказывает, если строк больше предела.
pub(crate) fn ensure_rows(count: usize, max: usize) -> Result<(), CoreError> {
    if count > max {
        return Err(CoreError::Validation(vec![FieldError::new(
            "rows",
            "too_many_rows",
        )]));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_over_the_limit_are_refused() {
        match ensure_rows(3, 2) {
            Err(CoreError::Validation(errors)) => {
                assert_eq!(errors, vec![FieldError::new("rows", "too_many_rows")]);
            }
            other => panic!("{other:?}"),
        }
        assert!(ensure_rows(2, 2).is_ok());
        assert!(ensure_rows(0, 2).is_ok());
    }
}
