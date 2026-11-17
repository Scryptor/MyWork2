//! Типы самого HTTP-слоя.

use std::fmt;

use chrono::NaiveDateTime;
use mw_core::User;
use mw_domain::permission::PermissionSet;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use utoipa::{IntoParams, ToSchema};

/// Сведения о сервере; доступны без входа.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
pub struct ServerInfo {
    /// Всегда `mw`.
    pub app: String,
    /// Версия сервера.
    pub version: String,
    /// Версия API (путь `/api/v{N}`).
    pub api_version: u32,
    /// Нужен ли вход: `false` только в режиме без входа на локальном адресе.
    pub auth_required: bool,
    /// Есть ли в базе активные пользователи.
    pub has_users: bool,
    /// Текущее время предприятия (местное, без смещения), как все даты API.
    pub server_time: String,
}

/// Вход в программу.
#[derive(Clone, Deserialize, TS, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct LoginRequest {
    /// Логин; регистр и крайние пробелы не важны.
    pub login: String,
    /// Пароль.
    pub password: String,
    /// Название устройства для списка сеансов (например, «Касса»); может быть пустым.
    pub device_label: String,
}

impl fmt::Debug for LoginRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoginRequest")
            .field("login", &self.login)
            .field("password", &"***")
            .field("device_label", &self.device_label)
            .finish()
    }
}

/// Результат входа. Токен показывается только один раз.
#[derive(Clone, Serialize, Deserialize, TS, ToSchema)]
pub struct LoginResponse {
    /// Токен для заголовка `Authorization: Bearer <токен>` (64 hex-символа).
    pub token: String,
    /// Пользователь.
    pub user: User,
    /// Права пользователя.
    pub permissions: PermissionSet,
}

impl fmt::Debug for LoginResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoginResponse")
            .field("token", &"***")
            .field("user", &self.user)
            .field("permissions", &self.permissions)
            .finish()
    }
}

/// Кто делает запросы.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
pub struct CurrentUser {
    /// Номер пользователя; пусто в режиме без входа.
    #[schema(required = true)]
    pub user_id: Option<i64>,
    /// Имя для показа и журнала действий.
    pub name: String,
    /// Права.
    pub permissions: PermissionSet,
    /// Работа без входа (сервер на локальном адресе, пользователей нет).
    pub open_mode: bool,
}

/// Смена собственного пароля.
#[derive(Clone, Deserialize, TS, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangePasswordRequest {
    /// Текущий пароль.
    pub current_password: String,
    /// Новый пароль (не короче 4 символов).
    pub new_password: String,
}

impl fmt::Debug for ChangePasswordRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChangePasswordRequest")
            .field("current_password", &"***")
            .field("new_password", &"***")
            .finish()
    }
}

/// Параметр списков с архивными записями.
#[derive(Debug, Clone, Default, Deserialize, TS, IntoParams)]
#[serde(deny_unknown_fields)]
#[ts(optional_fields)]
#[into_params(parameter_in = Query)]
pub struct IncludeArchived {
    /// Показывать и архивные записи; по умолчанию `false`.
    pub include_archived: Option<bool>,
}

/// Новый порядок записей: все их номера в нужной последовательности.
#[derive(Debug, Clone, Deserialize, TS, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ReorderRequest {
    /// Номера записей в новом порядке; должны совпадать с текущим набором.
    pub ordered_ids: Vec<i64>,
}

/// Значение справочника.
#[derive(Debug, Clone, Deserialize, TS, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DictionaryValue {
    /// Текст значения; крайние пробелы убираются.
    pub value: String,
}

/// Новая точка обслуживания.
#[derive(Debug, Clone, Deserialize, TS, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewServicePoint {
    /// Название точки.
    pub name: String,
}

/// Параметры списка и поиска клиентов.
#[derive(Debug, Clone, Default, Deserialize, TS, IntoParams)]
#[serde(deny_unknown_fields)]
#[ts(optional_fields)]
#[into_params(parameter_in = Query)]
pub struct ClientListQuery {
    /// Часть имени или телефона; без учёта регистра. Пусто — все клиенты.
    pub query: Option<String>,
    /// Размер страницы, 1..=500; по умолчанию 50.
    pub limit: Option<u64>,
    /// Сколько записей пропустить; по умолчанию 0.
    pub offset: Option<u64>,
}

/// Телефон для поиска клиентов; в любом написании.
#[derive(Debug, Clone, Deserialize, TS, IntoParams)]
#[serde(deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct PhoneQuery {
    /// Телефон, например `8 921 123-45-67`.
    pub phone: String,
}

/// Имя загружаемого файла.
#[derive(Debug, Clone, Deserialize, TS, IntoParams)]
#[serde(deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct AttachmentNameQuery {
    /// Имя файла, как у пользователя: 1–255 знаков.
    pub name: String,
}

/// Правка подписи вложения.
#[derive(Debug, Clone, Deserialize, TS, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CaptionPatch {
    /// Новая подпись; пустая строка очищает.
    pub caption: String,
}

/// Версия записи, которую правит клиент.
#[derive(Debug, Clone, Deserialize, TS, IntoParams)]
#[serde(deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct VersionParam {
    /// Версия записи, которую правит клиент.
    pub version: i64,
}

/// Страница клиентов.
#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema)]
pub struct ClientPage {
    /// Клиенты страницы.
    pub items: Vec<mw_core::Client>,
    /// Сколько клиентов подходит всего.
    pub total: u64,
}

impl From<mw_core::Page<mw_core::Client>> for ClientPage {
    fn from(p: mw_core::Page<mw_core::Client>) -> Self {
        Self {
            items: p.items,
            total: p.total,
        }
    }
}

/// Параметры списка заказов: фильтры, сортировка и страница.
#[derive(Debug, Clone, Default, Deserialize, TS, IntoParams)]
#[serde(deny_unknown_fields)]
#[ts(optional_fields)]
#[into_params(parameter_in = Query)]
pub struct OrderListQuery {
    /// Слова поиска по тексту заказа и клиента; все слова должны найтись. Пусто — без поиска.
    pub query: Option<String>,
    /// Только эти статусы (параметр можно повторять); пусто — любые.
    pub status_ids: Option<Vec<i64>>,
    /// Только эта точка обслуживания.
    pub service_point_id: Option<i64>,
    /// Только заказы этого клиента.
    pub client_id: Option<i64>,
    /// Нужно ли звонить клиенту; не указано — любые.
    pub need_call: Option<bool>,
    /// Нужны ли запчасти; не указано — любые.
    pub need_parts: Option<bool>,
    /// Выданные или находящиеся в сервисе; по умолчанию любые.
    pub issued: Option<mw_core::IssuedFilter>,
    /// Только с просроченной диагностикой; по умолчанию `false`.
    pub overdue_diagnostics: Option<bool>,
    /// Показывать и удалённые (право `orders_delete`); по умолчанию `false`.
    pub include_deleted: Option<bool>,
    /// Только этот тип устройства (без учёта регистра).
    pub device_type: Option<String>,
    /// Только этот бренд (без учёта регистра).
    pub brand: Option<String>,
    /// Только этот мастер (без учёта регистра).
    pub master: Option<String>,
    /// Принят не раньше (включительно), местное время без смещения.
    pub received_from: Option<NaiveDateTime>,
    /// Принят не позже (включительно), местное время без смещения.
    pub received_to: Option<NaiveDateTime>,
    /// Выдан не раньше (включительно), местное время без смещения.
    pub issued_from: Option<NaiveDateTime>,
    /// Выдан не позже (включительно), местное время без смещения.
    pub issued_to: Option<NaiveDateTime>,
    /// Только «давно готовые»; по умолчанию `false`.
    pub stale_ready: Option<bool>,
    /// Поле сортировки; по умолчанию дата приёма, новые сверху.
    pub sort: Option<mw_core::OrderSortField>,
    /// Обратный порядок; действует только вместе с `sort`, по умолчанию `false`.
    pub descending: Option<bool>,
    /// Размер страницы, 1..=500; по умолчанию 50.
    pub limit: Option<u64>,
    /// Сколько записей пропустить; по умолчанию 0.
    pub offset: Option<u64>,
}

impl OrderListQuery {
    /// Фильтр и порядок для ядра.
    pub fn into_filter_and_sort(self) -> (mw_core::OrderFilter, mw_core::OrderSort) {
        let sort = match self.sort {
            None => mw_core::OrderSort::default(),
            Some(field) => mw_core::OrderSort {
                field,
                descending: self.descending.unwrap_or(false),
            },
        };
        let filter = mw_core::OrderFilter {
            query: self.query.unwrap_or_default(),
            status_ids: self.status_ids.unwrap_or_default(),
            service_point_id: self.service_point_id,
            client_id: self.client_id,
            need_call: self.need_call,
            need_parts: self.need_parts,
            issued: self.issued.unwrap_or_default(),
            overdue_diagnostics: self.overdue_diagnostics.unwrap_or(false),
            include_deleted: self.include_deleted.unwrap_or(false),
            device_type: self.device_type,
            brand: self.brand,
            master: self.master,
            received_from: self.received_from,
            received_to: self.received_to,
            issued_from: self.issued_from,
            issued_to: self.issued_to,
            stale_ready: self.stale_ready.unwrap_or(false),
        };
        (filter, sort)
    }
}

/// Сводка заказов.
#[derive(Debug, Clone, Deserialize, TS, IntoParams)]
#[serde(deny_unknown_fields)]
#[ts(optional_fields)]
#[into_params(parameter_in = Query)]
pub struct OrderSummaryQuery {
    /// Только эта точка обслуживания.
    pub service_point_id: Option<i64>,
}

/// Подсказки значений поля заказа.
#[derive(Debug, Clone, Deserialize, TS, IntoParams)]
#[serde(deny_unknown_fields)]
#[ts(optional_fields)]
#[into_params(parameter_in = Query)]
pub struct OrderValuesQuery {
    /// Поле, значения которого нужны.
    pub field: mw_core::OrderValueField,
    /// Подстрока значения (без учёта регистра); пусто — все значения.
    pub query: Option<String>,
    /// Для моделей: только этого бренда.
    pub brand: Option<String>,
}

/// Штрихкод для поиска заказа.
#[derive(Debug, Clone, Deserialize, TS, IntoParams)]
#[serde(deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct BarcodeQuery {
    /// Считанный штрихкод; ведущий ноль и перевод строки сканера допускаются.
    pub code: String,
}

/// Смена статуса заказа.
#[derive(Debug, Clone, Deserialize, TS, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct StatusChange {
    /// Номер нового статуса.
    pub status_id: i64,
    /// Ставить ли дату выдачи при переходе в вид «выдан»; по умолчанию да.
    #[serde(default = "default_true")]
    #[schema(default = true)]
    #[ts(as = "Option<bool>", optional)]
    pub stamp_issued_at: bool,
}

fn default_true() -> bool {
    true
}

/// Страница списка: размер и смещение.
#[derive(Debug, Clone, Default, Deserialize, TS, IntoParams)]
#[serde(deny_unknown_fields)]
#[ts(optional_fields)]
#[into_params(parameter_in = Query)]
pub struct PageQuery {
    /// Размер страницы, 1..=500; по умолчанию 50.
    pub limit: Option<u64>,
    /// Сколько записей пропустить; по умолчанию 0.
    pub offset: Option<u64>,
}

/// Следующий номер заказа.
#[derive(Debug, Clone, Deserialize, TS, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NextOrderNumber {
    /// Номер, который получит следующий заказ; больше всех существующих.
    pub next: i64,
}

/// Страница заказов.
#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema)]
pub struct OrderPage {
    /// Заказы страницы.
    pub items: Vec<mw_core::OrderListItem>,
    /// Сколько заказов подходит всего.
    pub total: u64,
}

impl From<mw_core::Page<mw_core::OrderListItem>> for OrderPage {
    fn from(p: mw_core::Page<mw_core::OrderListItem>) -> Self {
        Self {
            items: p.items,
            total: p.total,
        }
    }
}

/// Новый пароль пользователя, который задаёт администратор.
#[derive(Clone, Deserialize, TS, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SetPasswordRequest {
    /// Новый пароль (не короче 4 символов).
    pub password: String,
}

impl fmt::Debug for SetPasswordRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SetPasswordRequest")
            .field("password", &"***")
            .finish()
    }
}

/// Параметры чтения журнала: фильтры и страница. Время — местное, `ГГГГ-ММ-ДДTЧЧ:ММ:СС`.
#[derive(Debug, Clone, Default, Deserialize, TS, IntoParams)]
#[serde(deny_unknown_fields)]
#[ts(optional_fields)]
#[into_params(parameter_in = Query)]
pub struct AuditListQuery {
    /// Только записи этого заказа.
    pub order_id: Option<i64>,
    /// Только записи этого клиента.
    pub client_id: Option<i64>,
    /// Только действия этого пользователя.
    pub user_id: Option<i64>,
    /// Только действия, код которых начинается с этой строки, например `order.`.
    pub action_prefix: Option<String>,
    /// Не раньше этого времени.
    pub from: Option<chrono::NaiveDateTime>,
    /// Не позже этого времени.
    pub to: Option<chrono::NaiveDateTime>,
    /// Размер страницы, 1..=500; по умолчанию 50.
    pub limit: Option<u64>,
    /// Сколько записей пропустить; по умолчанию 0.
    pub offset: Option<u64>,
}

impl From<AuditListQuery> for mw_core::AuditFilter {
    fn from(q: AuditListQuery) -> Self {
        Self {
            order_id: q.order_id,
            client_id: q.client_id,
            user_id: q.user_id,
            action_prefix: q.action_prefix,
            from: q.from,
            to: q.to,
        }
    }
}

/// Страница журнала.
#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema)]
pub struct AuditPage {
    /// Записи страницы, новые сверху.
    pub items: Vec<mw_core::AuditEntry>,
    /// Сколько записей подходит всего.
    pub total: u64,
}

impl From<mw_core::Page<mw_core::AuditEntry>> for AuditPage {
    fn from(p: mw_core::Page<mw_core::AuditEntry>) -> Self {
        Self {
            items: p.items,
            total: p.total,
        }
    }
}
