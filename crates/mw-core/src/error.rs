use mw_domain::document_template::CodeDiagnostic;
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
    /// Свой Typst-код шаблона не компилируется: ошибки с местом в коде, не больше 20.
    #[error("ошибка в своём Typst-коде: {}", code_summary(.0))]
    TemplateCode(Vec<CodeDiagnostic>),
    /// Запись используется и поэтому не удаляется; текст — для человека.
    #[error("{0}")]
    InUse(String),
    #[error("недостаточно прав: {}", .0.label_ru())]
    Forbidden(Permission),
    #[error("неверный логин или пароль")]
    InvalidCredentials,
    #[error("Программа уже настроена")]
    AlreadySetUp,
    #[error("требуется вход в систему")]
    Unauthenticated,
    #[error("{0}")]
    Unavailable(&'static str),
    #[error("внутренняя ошибка: {0}")]
    Internal(String),
    #[error("ошибка базы данных: {0}")]
    Db(#[from] DbErr),
}

/// Первая ошибка своего кода с местом и сколько их ещё: «unknown variable: nope (строка 1, знак 2;
/// ещё 1)».
fn code_summary(diagnostics: &[CodeDiagnostic]) -> String {
    let Some(first) = diagnostics.first() else {
        return "без подробностей".to_owned();
    };
    let mut notes = Vec::new();
    if let Some(place) = &first.place {
        notes.push(format!("строка {}, знак {}", place.line, place.column));
    }
    if diagnostics.len() > 1 {
        notes.push(format!("ещё {}", diagnostics.len() - 1));
    }
    if notes.is_empty() {
        first.message.clone()
    } else {
        format!("{} ({})", first.message, notes.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use mw_domain::document_template::CodePlace;

    use super::*;

    #[test]
    fn template_code_error_names_the_first_error_and_its_place() {
        let error = |place: Option<CodePlace>| CodeDiagnostic {
            place,
            message: "unknown variable: nope".into(),
            hints: Vec::new(),
        };
        let at = CodePlace {
            block: vec![2],
            line: 1,
            column: 2,
        };
        assert_eq!(
            CoreError::TemplateCode(vec![error(Some(at)), error(None)]).to_string(),
            "ошибка в своём Typst-коде: unknown variable: nope (строка 1, знак 2; ещё 1)"
        );
        assert_eq!(
            CoreError::TemplateCode(vec![error(None)]).to_string(),
            "ошибка в своём Typst-коде: unknown variable: nope"
        );
        assert_eq!(
            CoreError::TemplateCode(Vec::new()).to_string(),
            "ошибка в своём Typst-коде: без подробностей"
        );
    }
}
