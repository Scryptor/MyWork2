//! Предупреждения отчёта об импорте: коды стабильны, тесты сверяют их.

use serde::Serialize;

/// Уровень предупреждения.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Warning,
    Info,
}

macro_rules! warning_codes {
    ($( $variant:ident => ($code:literal, $severity:ident, $title:literal) ),+ $(,)?) => {
        /// Код предупреждения отчёта.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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
    StatusRenamed => ("status_renamed", Info, "Статус переименован"),
    StatusCustom => ("status_custom", Info, "Добавлен статус из старой базы"),
    PhoneSplit => ("phone_split", Info, "Телефоны разложены по номерам"),
    ClientDuplicates => ("client_duplicates", Info, "Похожие клиенты (одинаковые ФИО и телефон)"),
    BarcodeGenerated => ("barcode_generated", Info, "Создан штрихкод"),
    TrackingFound => ("tracking_found", Info, "Найдены трек-номера"),
    RoleRenamed => ("role_renamed", Info, "Группа переименована"),
    DictionaryPlaceholderSkipped => ("dictionary_placeholder_skipped", Info, "Пропущены строки-подсказки"),
    DictionaryDefaultKept => ("dictionary_default_kept", Info, "Список не менялся — взят стандартный"),
    ActTextDefaultSkipped => ("act_text_default_skipped", Info, "Заготовка текста акта не перенесена"),
    TextDecodedUtf8 => ("text_decoded_utf8", Info, "Файл прочитан как UTF-8"),
    SettingPreserved => ("setting_preserved", Info, "Настройка сохранена до появления в программе"),
    StockPreserved => ("stock_preserved", Info, "Склад сохранён до модуля склада"),
    ClientFilesOrphan => ("client_files_orphan", Info, "Файлы без заказа"),
    NextNumber => ("next_number", Info, "Следующий номер заказа"),
}

/// Одно предупреждение отчёта.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Warning {
    pub code: WarningCode,
    /// `Catalog`, `ClientsMap`, `StatesMap`, `HistoryBD`, `Users`, `GroupDostup`, `Stock`, `Config.ini`, `file`.
    pub source: String,
    pub key: String,
    pub field: String,
    pub value: String,
    pub message: String,
}

/// Накопитель предупреждений в порядке появления.
#[derive(Debug, Default)]
pub(crate) struct Warnings(Vec<Warning>);

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_unique_and_levels_follow_the_table() {
        let mut seen = std::collections::BTreeSet::new();
        for code in WarningCode::ALL {
            assert!(seen.insert(code.as_str()), "{}", code.as_str());
            assert!(!code.title_ru().is_empty());
        }
        assert_eq!(WarningCode::ALL.len(), 42);
        assert_eq!(WarningCode::FileUnreadable.severity(), Severity::Warning);
        assert_eq!(WarningCode::StatusRenamed.severity(), Severity::Info);
        assert_eq!(WarningCode::NextNumber.severity(), Severity::Info);
    }
}
