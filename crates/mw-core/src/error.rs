use mw_domain::permission::Permission;
use mw_domain::validation::FieldError;
use sea_orm::DbErr;

/// Ошибка прикладного слоя. Сообщения предназначены людям, поэтому по-русски.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("не найдено: {entity} №{id}")]
    NotFound { entity: &'static str, id: i64 },
    #[error(
        "запись изменена другим пользователем: {entity} №{id} (текущая версия {current_version})"
    )]
    Conflict {
        entity: &'static str,
        id: i64,
        current_version: i64,
    },
    #[error("некорректные данные: {}", .0.iter().map(|e| format!("{}: {}", e.field, e.code)).collect::<Vec<_>>().join(", "))]
    Validation(Vec<FieldError>),
    #[error("недостаточно прав: {}", .0.label_ru())]
    Forbidden(Permission),
    #[error("неверный логин или пароль")]
    InvalidCredentials,
    #[error("требуется вход в систему")]
    Unauthenticated,
    #[error("{0}")]
    Unavailable(&'static str),
    #[error("внутренняя ошибка: {0}")]
    Internal(String),
    #[error("ошибка базы данных: {0}")]
    Db(#[from] DbErr),
}
