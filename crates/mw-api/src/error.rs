//! Единый формат ошибок API: всегда JSON `ErrorBody`, сообщения по-русски.

use axum::extract::rejection::{BytesRejection, JsonRejection, PathRejection};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum_extra::extract::QueryRejection;
use mw_core::CoreError;
use mw_domain::permission::Permission;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use utoipa::ToSchema;

/// Машинный код ошибки.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    BadRequest,
    Unauthenticated,
    InvalidCredentials,
    Forbidden,
    NotFound,
    Conflict,
    Validation,
    TooManyAttempts,
    Unavailable,
    Internal,
}

/// Ошибка в поле формы.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
pub struct ErrorField {
    /// Имя поля.
    pub field: String,
    /// Машинный код (`required`, `invalid`, …).
    pub code: String,
    /// Сообщение по-русски.
    pub message: String,
}

/// Тело любого ответа с ошибкой.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
pub struct ErrorBody {
    pub code: ErrorCode,
    /// Сообщение по-русски, пригодное для показа человеку.
    pub message: String,
    /// Ошибки полей (только для `validation`).
    pub fields: Vec<ErrorField>,
    /// Текущая версия записи (только для `conflict`).
    #[schema(required = true)]
    pub current_version: Option<i64>,
    /// Недостающее право (только для `forbidden`).
    #[schema(required = true)]
    pub permission: Option<Permission>,
    /// Через сколько секунд повторить (только для `too_many_attempts`).
    #[schema(required = true)]
    pub retry_after_secs: Option<u64>,
}

/// Ошибка обработчика: статус и тело.
#[derive(Debug)]
pub(crate) struct ApiError {
    pub(crate) status: StatusCode,
    pub(crate) body: ErrorBody,
}

impl ApiError {
    fn new(status: StatusCode, code: ErrorCode, message: impl Into<String>) -> Self {
        ApiError {
            status,
            body: ErrorBody {
                code,
                message: message.into(),
                fields: Vec::new(),
                current_version: None,
                permission: None,
                retry_after_secs: None,
            },
        }
    }

    /// Заказ из пачки документов не найден: в сообщении его номер.
    pub(crate) fn order_not_found(id: i64) -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            ErrorCode::NotFound,
            format!("Заказ № {id} не найден"),
        )
    }

    pub(crate) fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, ErrorCode::BadRequest, message)
    }

    /// Запись о файле есть, а самого файла на диске нет.
    pub(crate) fn file_missing() -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            ErrorCode::NotFound,
            "Файл вложения не найден на диске",
        )
    }

    pub(crate) fn path_not_found() -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            ErrorCode::NotFound,
            "Нет такого адреса API",
        )
    }

    pub(crate) fn method_not_allowed() -> Self {
        Self::new(
            StatusCode::METHOD_NOT_ALLOWED,
            ErrorCode::BadRequest,
            "Метод не поддерживается для этого адреса",
        )
    }

    pub(crate) fn json_required() -> Self {
        Self::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ErrorCode::BadRequest,
            "Нужен заголовок Content-Type: application/json",
        )
    }

    pub(crate) fn host_forbidden() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            ErrorCode::Forbidden,
            "Недопустимое имя сервера в запросе",
        )
    }

    /// Импорт по пути на диске сервера — только серверу на loopback (локальный режим, десктоп).
    pub(crate) fn import_local_only() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            ErrorCode::Forbidden,
            "Импорт по пути доступен только на этом компьютере",
        )
    }

    pub(crate) fn import_running() -> Self {
        Self::new(StatusCode::CONFLICT, ErrorCode::Conflict, "Импорт уже идёт")
    }

    pub(crate) fn no_data_dir() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::Unavailable,
            "Папка данных сервера не задана",
        )
    }

    pub(crate) fn timed_out() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::Unavailable,
            "Сервер не успел обработать запрос, повторите",
        )
    }

    /// Слишком много неудачных попыток; `secs` — через сколько повторить.
    pub(crate) fn too_many_attempts(secs: u64) -> Self {
        let minutes = secs.div_ceil(60).max(1);
        let mut e = Self::new(
            StatusCode::TOO_MANY_REQUESTS,
            ErrorCode::TooManyAttempts,
            format!("Слишком много неудачных попыток. Повторите через {minutes} мин."),
        );
        e.body.retry_after_secs = Some(secs);
        e
    }

    /// Отказ разбора запроса: статус отказа axum, но 422 → 400.
    fn rejection(status: StatusCode, text: String) -> Self {
        let status = if status == StatusCode::UNPROCESSABLE_ENTITY {
            StatusCode::BAD_REQUEST
        } else {
            status
        };
        Self::new(
            status,
            ErrorCode::BadRequest,
            format!("Некорректный запрос: {text}"),
        )
    }
}

impl From<CoreError> for ApiError {
    fn from(err: CoreError) -> Self {
        match err {
            CoreError::NotFound { entity, id } => Self::new(
                StatusCode::NOT_FOUND,
                ErrorCode::NotFound,
                format!("Не найдено: {entity} № {id}"),
            ),
            CoreError::Conflict {
                entity,
                id,
                current_version,
            } => {
                let mut e = Self::new(
                    StatusCode::CONFLICT,
                    ErrorCode::Conflict,
                    format!(
                        "Запись изменена другим пользователем: {entity} № {id}. Обновите данные"
                    ),
                );
                e.body.current_version = Some(current_version);
                e
            }
            CoreError::Validation(fields) => {
                let mut e = Self::new(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    ErrorCode::Validation,
                    "Проверьте введённые данные",
                );
                e.body.fields = fields
                    .into_iter()
                    .map(|f| ErrorField {
                        message: f.message_ru().to_owned(),
                        field: f.field,
                        code: f.code,
                    })
                    .collect();
                e
            }
            CoreError::InUse(message) => {
                Self::new(StatusCode::CONFLICT, ErrorCode::Conflict, message)
            }
            CoreError::AlreadySetUp => Self::new(
                StatusCode::CONFLICT,
                ErrorCode::Conflict,
                "Программа уже настроена",
            ),
            CoreError::Forbidden(p) => {
                let mut e = Self::new(
                    StatusCode::FORBIDDEN,
                    ErrorCode::Forbidden,
                    format!("Недостаточно прав: {}", p.label_ru()),
                );
                e.body.permission = Some(p);
                e
            }
            CoreError::InvalidCredentials => Self::new(
                StatusCode::UNAUTHORIZED,
                ErrorCode::InvalidCredentials,
                "Неверный логин или пароль",
            ),
            CoreError::Unauthenticated => Self::new(
                StatusCode::UNAUTHORIZED,
                ErrorCode::Unauthenticated,
                "Требуется вход в программу",
            ),
            CoreError::Unavailable(message) => Self::new(
                StatusCode::SERVICE_UNAVAILABLE,
                ErrorCode::Unavailable,
                message,
            ),
            err @ (CoreError::Db(_) | CoreError::Internal(_)) => {
                tracing::error!(error = %err, "внутренняя ошибка");
                Self::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ErrorCode::Internal,
                    "Внутренняя ошибка сервера",
                )
            }
        }
    }
}

impl From<JsonRejection> for ApiError {
    fn from(r: JsonRejection) -> Self {
        Self::rejection(r.status(), r.body_text())
    }
}

impl From<BytesRejection> for ApiError {
    fn from(r: BytesRejection) -> Self {
        Self::rejection(r.status(), r.body_text())
    }
}

impl From<QueryRejection> for ApiError {
    fn from(r: QueryRejection) -> Self {
        Self::rejection(r.status(), r.body_text())
    }
}

impl From<PathRejection> for ApiError {
    fn from(r: PathRejection) -> Self {
        Self::rejection(r.status(), r.body_text())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status;
        let retry_after = self.body.retry_after_secs;
        let mut response = (status, axum::Json(self.body)).into_response();
        let headers = response.headers_mut();
        crate::middleware::set_security_headers(headers);
        if status == StatusCode::UNAUTHORIZED {
            headers.insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        }
        if status == StatusCode::TOO_MANY_REQUESTS
            && let Some(secs) = retry_after
        {
            headers.insert(header::RETRY_AFTER, HeaderValue::from(secs));
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mw_domain::validation::FieldError;
    use serde_json::json;

    #[test]
    fn core_errors_map_to_statuses_and_russian_messages() {
        let cases = [
            (
                CoreError::NotFound {
                    entity: "заказ",
                    id: 5,
                },
                404,
                "not_found",
                "Не найдено: заказ № 5",
            ),
            (
                CoreError::Conflict {
                    entity: "клиент",
                    id: 7,
                    current_version: 3,
                },
                409,
                "conflict",
                "Запись изменена другим пользователем: клиент № 7. Обновите данные",
            ),
            (
                CoreError::Forbidden(Permission::OrdersEdit),
                403,
                "forbidden",
                "Недостаточно прав: Изменение заказов",
            ),
            (
                CoreError::InvalidCredentials,
                401,
                "invalid_credentials",
                "Неверный логин или пароль",
            ),
            (
                CoreError::Unauthenticated,
                401,
                "unauthenticated",
                "Требуется вход в программу",
            ),
            (
                CoreError::Internal("секретные детали".into()),
                500,
                "internal",
                "Внутренняя ошибка сервера",
            ),
        ];
        for (err, status, code, message) in cases {
            let e = ApiError::from(err);
            assert_eq!(
                (
                    e.status.as_u16(),
                    serde_json::to_value(e.body.code).unwrap(),
                    e.body.message.as_str()
                ),
                (status, json!(code), message)
            );
        }
        assert_eq!(
            ApiError::from(CoreError::Conflict {
                entity: "клиент",
                id: 7,
                current_version: 3
            })
            .body
            .current_version,
            Some(3)
        );
        let e = ApiError::from(CoreError::Validation(vec![FieldError::required("brand")]));
        assert_eq!(
            (e.status.as_u16(), e.body.fields[0].message.as_str()),
            (422, "Заполните поле")
        );
    }
}
