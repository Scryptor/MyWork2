//! Предупреждения отчёта об импорте: коды стабильны, тесты сверяют их.

use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;
use mw_core::ImportSummary;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::{Value, json};

use crate::source::SchemaSummary;

/// Уровень предупреждения.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Требует внимания.
    Warning,
    /// Сведения.
    Info,
}

macro_rules! warning_codes {
    ($( $variant:ident => ($code:literal, $severity:ident, $title:literal) ),+ $(,)?) => {
        /// Код предупреждения отчёта.
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema,
        )]
        #[serde(rename_all = "snake_case")]
        pub enum WarningCode {
            $( $variant, )+
        }

        impl WarningCode {
            /// Все коды в порядке таблицы плана.
            pub const ALL: &'static [WarningCode] = &[ $( WarningCode::$variant, )+ ];

            /// Стабильный код.
            pub fn as_str(self) -> &'static str {
                match self { $( Self::$variant => $code, )+ }
            }

            pub fn severity(self) -> Severity {
                match self { $( Self::$variant => Severity::$severity, )+ }
            }

            /// Заголовок в отчёте.
            pub fn title_ru(self) -> &'static str {
                match self { $( Self::$variant => $title, )+ }
            }
        }
    };
}

warning_codes! {
    DateUnparsed => ("date_unparsed", Warning, "Не разобрана дата"),
    DateMonthFirst => ("date_month_first", Warning, "Дата прочитана как «месяц/день»"),
    ReceivedAtMissing => ("received_at_missing", Warning, "Нет даты приёма — подставлена"),
    MoneyUnparsed => ("money_unparsed", Warning, "Не разобрана сумма"),
    AmountSuspicious => ("amount_suspicious", Warning, "Подозрительно большая сумма"),
    FlagUnrecognized => ("flag_unrecognized", Warning, "Не распознан флаг"),
    StatusEmpty => ("status_empty", Warning, "Заказ без статуса"),
    IssuedWithoutDate => ("issued_without_date", Warning, "Выдан без даты выдачи"),
    OrderWithoutClient => ("order_without_client", Warning, "Заказ без клиента"),
    ClientMissing => ("client_missing", Warning, "Клиент не найден — восстановлен из заказа"),
    ClientEmpty => ("client_empty", Warning, "Клиент без имени и телефона"),
    BarcodeDuplicate => ("barcode_duplicate", Warning, "Одинаковый штрихкод у нескольких заказов"),
    BarcodeInvalid => ("barcode_invalid", Warning, "Штрихкод не по формату UPC-A"),
    PhoneUnusual => ("phone_unusual", Warning, "Необычный номер телефона"),
    PhoneNoDigits => ("phone_no_digits", Warning, "Телефон без цифр перенесён в заметку"),
    HistoryOrphan => ("history_orphan", Warning, "История ссылается на несуществующий заказ"),
    LoginRenamed => ("login_renamed", Warning, "Логин переименован"),
    UserGroupMissing => ("user_group_missing", Warning, "Группа пользователя не найдена"),
    UserPasswordInvalid => ("user_password_invalid", Warning, "Пароль пользователя не перенесён"),
    UsersManageGranted => ("users_manage_granted", Warning, "Право «Пользователи и права» выдано роли"),
    ConfigSectionIgnored => ("config_section_ignored", Warning, "Пропущены настройки другого компьютера"),
    SettingInvalid => ("setting_invalid", Warning, "Не разобрана настройка"),
    TextInvalidUtf8 => ("text_invalid_utf8", Warning, "Испорченный текст в базе"),
    UnknownColumn => ("unknown_column", Warning, "Неизвестная колонка старой базы"),
    UnknownTable => ("unknown_table", Warning, "Неизвестная таблица старой базы"),
    FileMissing => ("file_missing", Warning, "Файл не найден"),
    FileUnreadable => ("file_unreadable", Warning, "Файл не прочитан"),
    QuantityUnparsed => ("quantity_unparsed", Warning, "Не разобрано количество"),
    StockRowSkipped => ("stock_row_skipped", Warning, "Позиция склада не перенесена"),
    StockPartOrphan => ("stock_part_orphan", Warning, "Запчасть заказа без позиции склада"),
    StockPartWithoutOrder => ("stock_part_without_order", Warning, "Списание со склада без заказа"),
    ExtraCostsKept => ("extra_costs_kept", Warning, "Расходы заказа оставлены как есть"),
    StatusRenamed => ("status_renamed", Info, "Статус переименован"),
    StatusCustom => ("status_custom", Info, "Добавлен статус из старой базы"),
    PhoneSplit => ("phone_split", Info, "Телефоны разложены по номерам"),
    ClientDuplicates => ("client_duplicates", Info, "Похожие клиенты (одинаковые ФИО и телефон)"),
    BarcodeGenerated => ("barcode_generated", Info, "Создан штрихкод"),
    TrackingFound => ("tracking_found", Info, "Найдены трек-номера"),
    RoleRenamed => ("role_renamed", Info, "Группа переименована"),
    DictionaryPlaceholderSkipped => ("dictionary_placeholder_skipped", Info, "Пропущены строки-подсказки"),
    DictionaryDefaultKept => ("dictionary_default_kept", Info, "Список не менялся — взят стандартный"),
    DictionaryFromOrders => ("dictionary_from_orders", Info, "Справочник собран из заказов и файлов"),
    MasterNameJunk => ("master_name_junk", Info, "Мастер не назначен: в имени нет букв"),
    ActTextDefaultSkipped => ("act_text_default_skipped", Info, "Заготовка текста акта не перенесена"),
    TextDecodedUtf8 => ("text_decoded_utf8", Info, "Файл прочитан как UTF-8"),
    SettingPreserved => ("setting_preserved", Info, "Настройка сохранена до появления в программе"),
    StockPreserved => ("stock_preserved", Info, "Склад сохранён до модуля склада"),
    ClientFilesOrphan => ("client_files_orphan", Info, "Файлы без заказа"),
    StockImported => ("stock_imported", Info, "Склад перенесён"),
    NextNumber => ("next_number", Info, "Следующий номер заказа"),
}

/// Одно предупреждение отчёта.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Warning {
    pub code: WarningCode,
    /// `Catalog`, `ClientsMap`, `StatesMap`, `HistoryBD`, `Users`, `GroupDostup`, `Stock`, `StockMap`,
    /// `Config.ini`, `file`, `dictionary`.
    pub source: String,
    pub key: String,
    pub field: String,
    pub value: String,
    pub message: String,
}

/// Накопитель предупреждений в порядке появления.
#[derive(Debug, Default)]
pub(crate) struct Warnings(Vec<Warning>);

impl From<Vec<Warning>> for Warnings {
    fn from(list: Vec<Warning>) -> Self {
        Self(list)
    }
}

impl Warnings {
    pub(crate) fn push(
        &mut self,
        code: WarningCode,
        source: &str,
        key: &str,
        field: &str,
        value: &str,
        message: &str,
    ) {
        self.0.push(Warning {
            code,
            source: source.to_owned(),
            key: key.to_owned(),
            field: field.to_owned(),
            value: value.to_owned(),
            message: message.to_owned(),
        });
    }

    pub(crate) fn list(&self) -> &[Warning] {
        &self.0
    }

    pub(crate) fn into_list(self) -> Vec<Warning> {
        self.0
    }

    /// Последнее предупреждение кода с этим ключом — чтобы дополнить его.
    pub(crate) fn find_mut(&mut self, code: WarningCode, key: &str) -> Option<&mut Warning> {
        self.0
            .iter_mut()
            .rev()
            .find(|w| w.code == code && w.key == key)
    }
}

/// Сколько файлов перенесено.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema,
)]
pub struct FileStats {
    /// Скопировано.
    pub copied: u64,
    /// Байт скопировано.
    pub bytes: u64,
    /// Не найдены или не прочитаны.
    pub missing: u64,
}

/// Итог импорта: что перенесено, что требует внимания.
#[derive(Debug, Clone, Serialize)]
pub struct ImportReport {
    #[serde(serialize_with = "time_text")]
    pub started_at: NaiveDateTime,
    #[serde(serialize_with = "path_text")]
    pub source: PathBuf,
    pub schema: SchemaSummary,
    pub summary: ImportSummary,
    pub files: FileStats,
    /// Папка `backups/<время>` с `mywork.db` и `attachments/`; только при замене.
    #[serde(serialize_with = "backup_text")]
    pub backup: Option<PathBuf>,
    pub warnings: Vec<Warning>,
}

fn time_text<S: Serializer>(t: &NaiveDateTime, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&t.format("%Y-%m-%dT%H:%M:%S").to_string())
}

fn path_text<S: Serializer>(p: &Path, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&p.to_string_lossy())
}

fn backup_text<S: Serializer>(p: &Option<PathBuf>, s: S) -> Result<S::Ok, S::Error> {
    match p {
        Some(p) => s.serialize_str(&p.to_string_lossy()),
        None => s.serialize_none(),
    }
}

/// Часть отчёта, известная до записи в базу (пишется в `legacy_imports.report`; сводку `summary`
/// добавляет `Core::import_bundle`). `staging` — имя промежуточной папки: по нему запуск сервера
/// узнаёт, что база этой папки записана.
pub(crate) fn stored_report(
    schema: &SchemaSummary,
    warnings: &[Warning],
    files: &FileStats,
    backup: Option<&Path>,
    staging: &str,
) -> Value {
    json!({
        "schema": schema,
        "files": files,
        "warnings": warnings,
        "backup": backup.map(|p| p.to_string_lossy()),
        "staging": staging,
    })
}

fn megabytes(bytes: u64) -> String {
    let tenths = (bytes * 10).div_ceil(1024 * 1024);
    format!("{}.{}", tenths / 10, tenths % 10)
}

impl ImportReport {
    pub fn count(&self, code: WarningCode) -> usize {
        self.warnings.iter().filter(|w| w.code == code).count()
    }

    pub fn to_json(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }

    /// Краткая сводка: только числа и заголовки, без значений из старой базы.
    pub fn summary_text(&self) -> String {
        let m = &self.summary;
        let mut out = String::from("Импорт из MyWork 2 завершён.\n");
        out.push_str(&format!("Версия базы: {}", self.schema.version_text()));
        out.push_str("\n\nПеренесено:\n");
        let lines: [(&str, u64); 15] = [
            ("заказов", m.orders),
            ("из них удалённых", m.deleted_orders),
            ("клиентов", m.clients),
            ("телефонов клиентов", m.client_phones),
            ("записей истории", m.history),
            ("трек-номеров", m.tracking_numbers),
            ("статусов", m.statuses),
            ("точек обслуживания", m.service_points),
            ("значений справочников", m.dictionary_items),
            ("ролей", m.roles),
            ("пользователей", m.users),
            ("мастеров создано", m.masters_created),
            ("пользователей отмечено «Мастер»", m.masters_marked),
            ("позиций склада", m.stock_items),
            ("запчастей в заказах", m.order_parts),
        ];
        for (title, n) in lines {
            out.push_str(&format!("  {title} — {n}\n"));
        }
        out.push_str(&format!(
            "  файлов — {} ({} МБ), не перенесено — {}\n",
            self.files.copied,
            megabytes(self.files.bytes),
            self.files.missing
        ));
        out.push_str(&format!("  следующий номер заказа — {}\n", m.next_order_id));
        if let Some(backup) = &self.backup {
            out.push_str(&format!("\nКопия прежних данных: {}\n", backup.display()));
        }
        for (severity, title) in [
            (Severity::Warning, "Требуют внимания"),
            (Severity::Info, "Сведения"),
        ] {
            let total = self
                .warnings
                .iter()
                .filter(|w| w.code.severity() == severity)
                .count();
            out.push_str(&format!("\n{title} — {total}\n"));
            for code in WarningCode::ALL.iter().filter(|c| c.severity() == severity) {
                let n = self.count(*code);
                if n > 0 {
                    out.push_str(&format!("  {} — {n}\n", code.title_ru()));
                }
            }
        }
        out
    }

    /// Сводка и все предупреждения построчно.
    pub fn full_text(&self) -> String {
        let mut out = self.summary_text();
        out.push_str(
            "\nПодробности (заголовок | источник | ключ | поле | значение | пояснение):\n",
        );
        for w in &self.warnings {
            let one_line = |s: &str| s.replace("\r\n", "\\n").replace(['\r', '\n'], "\\n");
            out.push_str(&format!(
                "{} | {} | {} | {} | {} | {}\n",
                w.code.title_ru(),
                one_line(&w.source),
                one_line(&w.key),
                one_line(&w.field),
                one_line(&w.value),
                one_line(&w.message)
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialized_code_equals_as_str_for_every_code() {
        for code in WarningCode::ALL {
            assert_eq!(
                serde_json::to_value(code).unwrap(),
                serde_json::Value::String(code.as_str().to_owned()),
                "{code:?}"
            );
        }
    }
    #[test]
    fn openapi_and_typescript_use_the_same_codes() {
        use ts_rs::TS;
        let codes: Vec<&str> = WarningCode::ALL.iter().map(|c| c.as_str()).collect();
        let schema =
            serde_json::to_value(<WarningCode as utoipa::PartialSchema>::schema()).unwrap();
        assert_eq!(schema["enum"], serde_json::json!(codes));
        let union = codes
            .iter()
            .map(|c| format!("\"{c}\""))
            .collect::<Vec<_>>()
            .join(" | ");
        assert_eq!(
            WarningCode::decl(&ts_rs::Config::new()),
            format!("type WarningCode = {union};")
        );
    }

    #[test]
    fn codes_are_unique_and_levels_follow_the_table() {
        let mut seen = std::collections::BTreeSet::new();
        for code in WarningCode::ALL {
            assert!(seen.insert(code.as_str()), "{}", code.as_str());
            assert!(!code.title_ru().is_empty());
        }
        assert_eq!(WarningCode::ALL.len(), 50);
        assert_eq!(WarningCode::FileUnreadable.severity(), Severity::Warning);
        assert_eq!(WarningCode::ExtraCostsKept.severity(), Severity::Warning);
        assert_eq!(WarningCode::StockImported.severity(), Severity::Info);
        assert_eq!(WarningCode::StatusRenamed.severity(), Severity::Info);
        assert_eq!(WarningCode::NextNumber.severity(), Severity::Info);
    }
}
