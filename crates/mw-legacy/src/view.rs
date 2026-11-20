//! Прошлые импорты для интерфейса: строка списка и отчёт с группами предупреждений.
//!
//! Строятся из сохранённого в `legacy_imports.report` JSON: схема, файлы, предупреждения, копия и
//! сводка записанного. Отчёт, записанный более старой версией, читается с нулями вместо недостающего.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;
use mw_core::{ImportSummary, LegacyImportMode, LegacyImportRecord};
use serde::{Deserialize, Serialize};

use crate::report::{FileStats, ImportReport, Severity, Warning, WarningCode};
use crate::source::SchemaSummary;

/// Строка списка прошлых импортов.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct LegacyImportItem {
    pub id: i64,
    pub imported_at: NaiveDateTime,
    /// Папка MyWork 2, из которой перенесено.
    pub source_path: String,
    pub mode: LegacyImportMode,
    /// Перенесено заказов.
    pub orders: u64,
    /// Предупреждений «требуют внимания».
    pub warnings: u64,
}

/// Группа предупреждений одного кода.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct ReportGroup {
    pub code: WarningCode,
    pub severity: Severity,
    /// Заголовок, как в текстовом отчёте.
    pub title: String,
    pub count: u64,
    /// Номера заказов, к которым относятся предупреждения группы, по возрастанию без повторов.
    pub order_ids: Vec<i64>,
    /// Предупреждения группы без номера заказа (клиенты, история, файлы, …): первые
    /// `REPORT_GROUP_ENTRIES` в порядке отчёта. Предупреждения заказов видны по `order_ids`.
    pub entries: Vec<ReportEntry>,
    /// Сколько всего предупреждений группы без номера заказа; `count` — все предупреждения группы.
    pub entries_count: u64,
}

/// Сколько предупреждений группы без заказа отдаёт отчёт для показа; полный список — в `warnings_text`.
pub const REPORT_GROUP_ENTRIES: usize = 200;

/// Одно предупреждение группы: что и где найдено в старой базе.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct ReportEntry {
    /// Таблица или файл старой базы (`Catalog`, `Users`, `file`, …).
    pub source: String,
    /// Номер записи, имя файла или другой ключ.
    pub key: String,
    /// Поле (колонка), если предупреждение о поле.
    pub field: String,
    /// Значение из старой базы.
    pub value: String,
    /// Что сделал перенос.
    pub message: String,
}

/// Отчёт прошлого импорта.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct ImportReportView {
    pub id: i64,
    pub imported_at: NaiveDateTime,
    pub source_path: String,
    pub mode: LegacyImportMode,
    /// Перенесено заказов.
    pub orders: u64,
    /// Перенесено клиентов.
    pub clients: u64,
    /// Перенесено пользователей.
    pub users: u64,
    /// Файлы: скопировано, байт, не найдено.
    pub files: FileStats,
    /// Группы по кодам: сначала «требуют внимания», затем сведения, в порядке таблицы кодов.
    pub groups: Vec<ReportGroup>,
    /// Полный текст отчёта, как в файле команды `import-legacy` (содержит данные клиентов).
    pub warnings_text: String,
    /// Файлы импорта ещё лежат в промежуточной папке (например, их место занято): их доведёт запуск
    /// программы или перенесёт человек.
    pub files_pending: bool,
}

/// Отчёт в `legacy_imports.report`.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct StoredReport {
    schema: SchemaSummary,
    files: FileStats,
    warnings: Vec<Warning>,
    backup: Option<String>,
    summary: ImportSummary,
    staging: Option<String>,
}

impl StoredReport {
    fn parse(record: &LegacyImportRecord) -> StoredReport {
        StoredReport::deserialize(&record.report).unwrap_or_else(|e| {
            tracing::warn!(import_id = record.id, error = %e, "отчёт импорта не разобран");
            StoredReport::default()
        })
    }

    fn warnings_of(&self, severity: Severity) -> u64 {
        self.warnings
            .iter()
            .filter(|w| w.code.severity() == severity)
            .count() as u64
    }
}

/// Строка списка по записи импорта.
pub fn import_item(record: &LegacyImportRecord) -> LegacyImportItem {
    let stored = StoredReport::parse(record);
    LegacyImportItem {
        id: record.id,
        imported_at: record.imported_at,
        source_path: record.source_path.clone(),
        mode: record.mode,
        orders: stored.summary.orders,
        warnings: stored.warnings_of(Severity::Warning),
    }
}

/// Отчёт по записи импорта: итоги, группы с номерами заказов и текст отчёта команды.
///
/// `attachments_dir` — папка вложений: по ней видно, осталась ли промежуточная папка этого импорта.
pub fn report_view(
    record: &LegacyImportRecord,
    attachments_dir: Option<&Path>,
) -> ImportReportView {
    let stored = StoredReport::parse(record);
    let files_pending = match (attachments_dir, stored.staging.as_deref()) {
        (Some(dir), Some(name)) if is_staging_name(name) => dir.join(name).is_dir(),
        _ => false,
    };
    let groups = WarningCode::ALL
        .iter()
        .filter_map(|code| {
            let list: Vec<&Warning> = stored.warnings.iter().filter(|w| w.code == *code).collect();
            if list.is_empty() {
                return None;
            }
            let mut orders = BTreeSet::new();
            let mut without_order = Vec::new();
            for w in &list {
                let ids = order_ids(w);
                if ids.is_empty() {
                    without_order.push(*w);
                }
                orders.extend(ids);
            }
            Some(ReportGroup {
                code: *code,
                severity: code.severity(),
                title: code.title_ru().to_owned(),
                count: list.len() as u64,
                order_ids: orders.into_iter().collect(),
                entries_count: without_order.len() as u64,
                entries: without_order
                    .iter()
                    .take(REPORT_GROUP_ENTRIES)
                    .map(|w| ReportEntry {
                        source: w.source.clone(),
                        key: w.key.clone(),
                        field: w.field.clone(),
                        value: w.value.clone(),
                        message: w.message.clone(),
                    })
                    .collect(),
            })
        })
        .collect();
    let report = ImportReport {
        started_at: record.imported_at,
        source: PathBuf::from(&record.source_path),
        schema: stored.schema,
        summary: stored.summary,
        files: stored.files,
        backup: stored.backup.map(PathBuf::from),
        warnings: stored.warnings,
    };
    ImportReportView {
        id: record.id,
        imported_at: record.imported_at,
        source_path: record.source_path.clone(),
        mode: record.mode,
        orders: report.summary.orders,
        clients: report.summary.clients,
        users: report.summary.users,
        files: report.files,
        groups,
        warnings_text: report.full_text(),
        files_pending,
    }
}

/// Имя промежуточной папки без путей: отчёт пишет сервер, но имя всё равно не доверяется.
fn is_staging_name(name: &str) -> bool {
    name.starts_with(".import-") && !name.contains(['/', '\\']) && !name.contains("..")
}

/// Номера заказов предупреждения: ключ, если источник — заказы (`Catalog`) и ключ — номер заказа.
///
/// У одинаковых штрихкодов ключ — сам штрихкод, а номера заказов перечислены в значении («заказы 3, 7»);
/// у переименованного и добавленного статуса ключ — название, у следующего номера — `Catalog`,
/// у неизвестной колонки — её имя.
fn order_ids(w: &Warning) -> Vec<i64> {
    if !w.source.eq_ignore_ascii_case("Catalog") {
        return Vec::new();
    }
    let number = |s: &str| s.trim().parse::<i64>().ok().filter(|id| *id > 0);
    match w.code {
        WarningCode::BarcodeDuplicate => w
            .value
            .strip_prefix("заказы ")
            .map(|list| list.split(',').filter_map(number).collect())
            .unwrap_or_default(),
        WarningCode::StatusRenamed
        | WarningCode::StatusCustom
        | WarningCode::NextNumber
        | WarningCode::UnknownColumn => Vec::new(),
        _ => number(&w.key).into_iter().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn warning(code: WarningCode, source: &str, key: &str, value: &str) -> Warning {
        Warning {
            code,
            source: source.to_owned(),
            key: key.to_owned(),
            field: String::new(),
            value: value.to_owned(),
            message: String::new(),
        }
    }

    #[test]
    fn order_ids_come_from_order_keys_only() {
        use WarningCode::*;
        let cases = [
            (warning(DateUnparsed, "Catalog", "7", "32.13.2015"), vec![7]),
            (warning(TextInvalidUtf8, "catalog", " 12 ", ""), vec![12]),
            (warning(TextInvalidUtf8, "Catalog", "строка 3", ""), vec![]),
            (
                warning(BarcodeDuplicate, "Catalog", "123456789012", "заказы 3, 7"),
                vec![3, 7],
            ),
            (warning(StatusCustom, "Catalog", "5", "5"), vec![]),
            (warning(StatusRenamed, "Catalog", "Ждет ЗИП", ""), vec![]),
            (warning(NextNumber, "Catalog", "Catalog", "16"), vec![]),
            (warning(UnknownColumn, "Catalog", "42", "1"), vec![]),
            (warning(HistoryOrphan, "HistoryBD", "4", "99"), vec![]),
            (warning(FileMissing, "file", "1/1/a.jpg", ""), vec![]),
            (warning(DateUnparsed, "Catalog", "-1", ""), vec![]),
        ];
        for (w, expected) in cases {
            assert_eq!(order_ids(&w), expected, "{w:?}");
        }
    }

    fn entry(source: &str, key: &str, value: &str) -> ReportEntry {
        ReportEntry {
            source: source.into(),
            key: key.into(),
            field: String::new(),
            value: value.into(),
            message: String::new(),
        }
    }

    #[test]
    fn a_group_gives_its_first_entries_in_report_order_and_counts_all() {
        let warnings: Vec<_> = (0..REPORT_GROUP_ENTRIES + 50)
            .map(|n| warning(WarningCode::FileMissing, "file", &format!("f{n}"), "v"))
            .chain([warning(WarningCode::NextNumber, "Catalog", "Catalog", "9")])
            .collect();
        let view = report_view(&record(json!({ "warnings": warnings })), None);
        let group = view
            .groups
            .iter()
            .find(|g| g.code == WarningCode::FileMissing)
            .unwrap();
        assert_eq!((group.count, group.entries_count), (250, 250));
        assert_eq!(group.entries.len(), REPORT_GROUP_ENTRIES);
        assert_eq!(group.entries[0].key, "f0");
        assert_eq!(group.entries[199].key, "f199");
        let next = view
            .groups
            .iter()
            .find(|g| g.code == WarningCode::NextNumber)
            .unwrap();
        assert_eq!((next.count, next.entries.len()), (1, 1));
    }

    #[test]
    fn entries_are_the_warnings_without_an_order() {
        // «Не разобрана дата» — и у заказов, и у клиентов: заказы идут номерами, остальное — записями.
        let warnings: Vec<_> = [
            warning(WarningCode::DateUnparsed, "Catalog", "5", "a"),
            warning(WarningCode::DateUnparsed, "Clients", "c0", "b"),
            warning(WarningCode::DateUnparsed, "Catalog", "3", "c"),
        ]
        .into_iter()
        .chain(
            (1..REPORT_GROUP_ENTRIES + 50)
                .map(|n| warning(WarningCode::DateUnparsed, "Clients", &format!("c{n}"), "v")),
        )
        .chain([warning(WarningCode::DateUnparsed, "Catalog", "9", "d")])
        .collect();
        let view = report_view(&record(json!({ "warnings": warnings })), None);
        let group = &view.groups[0];
        assert_eq!(group.code, WarningCode::DateUnparsed);
        assert_eq!(group.count, 253);
        assert_eq!(group.order_ids, [3, 5, 9]);
        assert_eq!(group.entries_count, 250);
        assert_eq!(group.entries.len(), REPORT_GROUP_ENTRIES);
        assert_eq!(group.entries[0], entry("Clients", "c0", "b"));
        assert_eq!(group.entries[199].key, "c199");
        assert!(group.entries.iter().all(|e| e.source == "Clients"));
    }

    fn record(report: serde_json::Value) -> LegacyImportRecord {
        LegacyImportRecord {
            id: 3,
            imported_at: chrono::NaiveDate::from_ymd_opt(2026, 11, 11)
                .unwrap()
                .and_hms_opt(9, 0, 0)
                .unwrap(),
            source_path: "/old".into(),
            mode: LegacyImportMode::Replace,
            report,
        }
    }

    #[test]
    fn groups_merge_order_numbers_and_follow_the_code_table() {
        let r = record(json!({
            "warnings": [
                warning(WarningCode::StatusCustom, "Catalog", "Свой", "Свой"),
                warning(WarningCode::DateUnparsed, "Catalog", "9", "x"),
                warning(WarningCode::DateUnparsed, "Catalog", "2", "y"),
                warning(WarningCode::DateUnparsed, "Catalog", "9", "z"),
            ],
            "summary": ImportSummary { orders: 4, clients: 2, users: 1, ..Default::default() },
        }));
        let view = report_view(&r, None);
        assert_eq!(
            view.groups,
            [
                ReportGroup {
                    code: WarningCode::DateUnparsed,
                    severity: Severity::Warning,
                    title: "Не разобрана дата".into(),
                    count: 3,
                    order_ids: vec![2, 9],
                    entries: vec![],
                    entries_count: 0,
                },
                ReportGroup {
                    code: WarningCode::StatusCustom,
                    severity: Severity::Info,
                    title: "Добавлен статус из старой базы".into(),
                    count: 1,
                    order_ids: vec![],
                    entries: vec![entry("Catalog", "Свой", "Свой")],
                    entries_count: 1,
                },
            ]
        );
        assert_eq!((view.orders, view.clients, view.users), (4, 2, 1));
        assert_eq!(view.mode, LegacyImportMode::Replace);
        let item = import_item(&r);
        assert_eq!((item.orders, item.warnings), (4, 3));
    }

    #[test]
    fn files_are_pending_while_the_staging_folder_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let r = record(json!({ "staging": ".import-20261111-090000" }));
        assert!(
            !report_view(&r, Some(dir.path())).files_pending,
            "папки нет"
        );
        std::fs::create_dir(dir.path().join(".import-20261111-090000")).unwrap();
        assert!(report_view(&r, Some(dir.path())).files_pending);
        assert!(
            !report_view(&r, None).files_pending,
            "папка вложений неизвестна"
        );
        let other = record(json!({ "staging": "../.import-x" }));
        assert!(
            !report_view(&other, Some(dir.path())).files_pending,
            "чужие пути не смотрим"
        );
        assert!(!report_view(&record(json!({})), Some(dir.path())).files_pending);
    }

    #[test]
    fn old_or_broken_reports_are_read_with_zeros() {
        for report in [json!({ "warnings": [] }), json!(null), json!("x")] {
            let view = report_view(&record(report), None);
            assert_eq!((view.orders, view.groups.len()), (0, 0));
            assert!(view.warnings_text.contains("Перенесено"));
            assert_eq!(import_item(&record(json!(null))).warnings, 0);
        }
    }
}
