//! Импорт из MyWork 2: типы пакета и запись в одной транзакции (`Core::import_bundle`),
//! список и чтение прошлых импортов (`Core::list_legacy_imports`, `Core::legacy_import`).
//!
//! Пакет собирает крейт `mw-legacy`; это не типы API (TypeScript и OpenAPI для них не выводятся),
//! кроме `LegacyImportMode`.

mod reports;
mod write;

pub use reports::{LegacyImportMode, LegacyImportRecord};

use std::path::PathBuf;

use chrono::NaiveDateTime;
use mw_domain::dictionary::DictionaryKind;
use mw_domain::money::Money;

use crate::{ClientPhone, NewRole, NewStatus, SettingsPatch};

/// Какая СУБД под `Core`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseKind {
    Sqlite,
    Postgres,
}

/// Режим импорта.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportMode {
    /// Только в пустую базу (без клиентов, заказов, пользователей и записей журнала).
    IntoEmpty,
    /// Заменить всё (только SQLite): сначала копия базы в `backup_to`.
    Replace { backup_to: PathBuf },
}

/// Что сейчас лежит в базе-приёмнике.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportTarget {
    pub clients: u64,
    pub orders: u64,
    pub users: u64,
    pub audit_entries: u64,
}

impl ImportTarget {
    /// База считается пустой, если в ней нет ни клиентов, ни заказов, ни пользователей, ни журнала.
    pub fn is_empty(&self) -> bool {
        self.clients == 0 && self.orders == 0 && self.users == 0 && self.audit_entries == 0
    }
}

/// Пользователь старой программы; пароль — SHA-1 в нижнем регистре.
#[derive(Clone, PartialEq, Eq)]
pub struct ImportedUser {
    pub login: String,
    /// Название роли из `ImportBundle::roles`.
    pub role: String,
    pub legacy_sha1: String,
}

impl std::fmt::Debug for ImportedUser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImportedUser")
            .field("login", &self.login)
            .field("role", &self.role)
            .field("legacy_sha1", &"***")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imported_user_debug_hides_the_hash() {
        let u = ImportedUser {
            login: "Ирина".into(),
            role: "Админы".into(),
            legacy_sha1: "7110eda4d09e062aa5e4a390b0a572ac0d2c0220".into(),
        };
        let text = format!("{u:?}");
        assert!(!text.contains("7110eda4"));
        assert!(text.contains("Ирина"));
    }
}

/// Настройки из старой программы. `values.timezone`, `values.default_service_point_id` и
/// `values.default_master_user_id` должны быть пусты.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportedSettings {
    pub values: SettingsPatch,
    /// Название точки из `ImportBundle::service_points`.
    pub default_service_point: Option<String>,
    /// Имя мастера по умолчанию: становится пользователем-мастером, как имена из
    /// `ImportBundle::masters`.
    pub default_master: Option<String>,
}

/// Клиент со своим номером.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedClient {
    pub id: i64,
    pub full_name: String,
    pub phones: Vec<ClientPhone>,
    pub address: String,
    pub note: String,
    pub source: String,
    pub blacklisted: bool,
    pub created_at: NaiveDateTime,
}

/// Заказ со своим номером; статус и точка — по названию.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedOrder {
    pub id: i64,
    pub client_id: i64,
    pub status: String,
    pub service_point: Option<String>,
    pub received_at: NaiveDateTime,
    pub issued_at: Option<NaiveDateTime>,
    pub prepaid_at: Option<NaiveDateTime>,
    pub device_type: String,
    pub brand: String,
    pub model: String,
    pub serial: String,
    pub color: String,
    pub completeness: String,
    pub device_condition: String,
    pub malfunction: String,
    pub work_done: String,
    pub internal_comment: String,
    pub master: String,
    pub barcode: String,
    pub estimate_from: Option<Money>,
    pub estimate_to: Option<Money>,
    pub final_price: Option<Money>,
    pub prepayment: Money,
    pub extra_costs: Money,
    pub discount: Money,
    pub warranty: String,
    pub need_call: bool,
    pub need_parts: bool,
    pub deleted_at: Option<NaiveDateTime>,
    /// (номер, описание).
    pub tracking: Vec<(String, String)>,
}

/// Запись журнала со своим временем и автором.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedHistory {
    pub at: NaiveDateTime,
    pub user_name: String,
    pub action: String,
    pub order_id: Option<i64>,
    pub client_id: Option<i64>,
    pub summary: String,
    pub details: Option<serde_json::Value>,
}

/// Куда попала исходная строка; роль и пользователь — по названию и логину.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacyTarget {
    Order(i64),
    Client(i64),
    Role(String),
    User(String),
}

/// След исходной строки: откуда, куда, полное исходное значение.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedLegacyRow {
    pub source: String,
    pub source_key: String,
    pub target: Option<LegacyTarget>,
    pub raw: serde_json::Value,
}

/// Всё, что нужно записать из старой программы.
#[derive(Debug, Clone)]
pub struct ImportBundle {
    pub source_path: String,
    /// В порядке `sort_order`.
    pub statuses: Vec<NewStatus>,
    /// В порядке `sort_order`.
    pub service_points: Vec<String>,
    /// Каждый вид справочника ровно один раз; мастера — не справочник (их список пуст).
    pub dictionary: Vec<(DictionaryKind, Vec<String>)>,
    /// Имена мастеров (файл мастеров MyWork 2). Они и имена мастеров из заказов становятся
    /// пользователями-мастерами: совпавший по имени или логину пользователь получает отметку
    /// «Мастер», для остальных создаются пользователи без входа (`mw_db::masters`).
    pub masters: Vec<String>,
    pub roles: Vec<NewRole>,
    pub users: Vec<ImportedUser>,
    pub settings: ImportedSettings,
    pub clients: Vec<ImportedClient>,
    pub orders: Vec<ImportedOrder>,
    /// В порядке записи (по времени).
    pub history: Vec<ImportedHistory>,
    pub legacy_rows: Vec<ImportedLegacyRow>,
    pub next_order_id: i64,
    pub next_client_id: i64,
    /// Пишется в `legacy_imports.report`; объект получает ещё ключ `summary` — `ImportSummary` записанного.
    pub report: serde_json::Value,
}

/// Итог импорта: сколько чего записано.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ImportSummary {
    pub import_id: i64,
    pub statuses: u64,
    pub service_points: u64,
    pub dictionary_items: u64,
    pub roles: u64,
    pub users: u64,
    pub clients: u64,
    pub client_phones: u64,
    pub orders: u64,
    pub deleted_orders: u64,
    pub tracking_numbers: u64,
    /// Только записи пакета, без записи `legacy.imported`.
    pub history: u64,
    pub legacy_rows: u64,
    pub next_order_id: i64,
    pub next_client_id: i64,
    /// Создано пользователей-мастеров без входа (в `users` не входят). В отчётах импортов до
    /// плана 5.8 этих чисел нет — читаются нулём.
    #[serde(default)]
    pub masters_created: u64,
    /// Пользователей MyWork 2, отмеченных «Мастер».
    #[serde(default)]
    pub masters_marked: u64,
}
