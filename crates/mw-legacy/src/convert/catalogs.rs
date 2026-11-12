//! Статусы, точки обслуживания и справочники из старой базы и файлов `settings/*.txt`.

use std::collections::{BTreeSet, HashMap};

use mw_core::NewStatus;
use mw_domain::dictionary::{DEFAULT_DICTIONARY, DictionaryKind};
use mw_domain::search::normalize_search;
use mw_domain::status::{DEFAULT_STATUSES, StatusKind};

use crate::folder::LegacyFolder;
use crate::report::{WarningCode, Warnings};
use crate::source::LegacyDb;
use crate::text::{TextEncoding, decode_text, text_lines};

/// Название статуса пустого значения.
pub(crate) const NO_STATUS: &str = "Без статуса";
const SETTINGS_PREFIX: &str = "settings/";

/// Стартовое содержимое файлов `settings/*.txt`, как его создаёт `Form1.FilesExistsOrNot`: (файл, строки).
pub(crate) const LEGACY_DEFAULT_FILES: &[(&str, &[&str])] = &[
    (
        "aboutUs.txt",
        &[
            "Наружняя реклама",
            "Интернет",
            "Этот текст можно",
            "поменять в файле",
            "settings/aboutUs.txt",
        ],
    ),
    (
        "AdresSC.txt",
        &[
            "Улица 3й ноги",
            "Переулок 2го уха",
            "Этот текст можно",
            "поменять в файле",
            "settings/AdresSC.txt",
        ],
    ),
    (
        "brands.txt",
        &[
            "ACER",
            "ASUS",
            "APPLE",
            "LENOVO",
            "SAMSUNG",
            "HEWLETT PACKARD",
            "DELL",
            "MSI",
            "DIGMA",
            "BENQ",
            "BBK",
        ],
    ),
    (
        "DeviceColour.txt",
        &["Белый", "Чёрный", "Серебристый", "Золотоистый", "Синий"],
    ),
    (
        "komplektonst.txt",
        &[
            "Аппарат",
            "АКБ",
            "Зарядное устройство",
            "Чехол",
            "Блок питания",
            "Этот текст можно",
            "поменять в файле",
            "settings/komplektonst.txt",
        ],
    ),
    (
        "masters.txt",
        &[
            "Мастер1",
            "Этот текст можно",
            "поменять в файле",
            "settings/masters.txt",
        ],
    ),
    (
        "neispravnost.txt",
        &[
            "Что-то сломано",
            "Этот текст можно",
            "поменять в файле",
            "settings/neispravnost.txt",
        ],
    ),
    (
        "sostoyaniePriema.txt",
        &[
            "Не бит, не крашен",
            "Этот текст можно",
            "поменять в файле",
            "settings/sostoyaniePriema.txt",
        ],
    ),
    (
        "ustrojstvo.txt",
        &[
            "Ноутбук",
            "Телефон",
            "Патифон",
            "Этот текст можно",
            "поменять в файле",
            "settings/ustrojstvo.txt",
        ],
    ),
    (
        "vipolnRaboti.txt",
        &[
            "Замена чего-нибудь",
            "Диагностика",
            "Этот текст можно",
            "поменять в файле",
            "settings/vipolnRaboti.txt",
        ],
    ),
];

/// Какой файл какому справочнику соответствует.
const DICTIONARY_FILES: &[(DictionaryKind, &str)] = &[
    (DictionaryKind::DeviceType, "ustrojstvo.txt"),
    (DictionaryKind::Brand, "brands.txt"),
    (DictionaryKind::Color, "DeviceColour.txt"),
    (DictionaryKind::Completeness, "komplektonst.txt"),
    (DictionaryKind::Condition, "sostoyaniePriema.txt"),
    (DictionaryKind::Malfunction, "neispravnost.txt"),
    (DictionaryKind::WorkDone, "vipolnRaboti.txt"),
    (DictionaryKind::Master, "masters.txt"),
    (DictionaryKind::ClientSource, "aboutUs.txt"),
    (DictionaryKind::Warranty, "Garanty.txt"),
];

/// Каталоги, собранные из старой базы и файлов настроек.
#[derive(Debug, Clone)]
pub(crate) struct Catalogs {
    pub statuses: Vec<NewStatus>,
    pub service_points: Vec<String>,
    pub dictionary: Vec<(DictionaryKind, Vec<String>)>,
    status_by_key: HashMap<String, String>,
    point_by_key: HashMap<String, String>,
}

impl Catalogs {
    /// Название статуса MyWork 3 для исходного текста (пустой — «Без статуса»).
    pub(crate) fn status_for(&self, legacy: &str) -> String {
        let trimmed = legacy.trim();
        let wanted = if trimmed.is_empty() {
            NO_STATUS
        } else {
            trimmed
        };
        self.status_by_key
            .get(&normalize_search(wanted))
            .cloned()
            .unwrap_or_else(|| wanted.to_owned())
    }

    /// Название точки MyWork 3 для исходного текста; пусто или неизвестная точка — `None`.
    pub(crate) fn point_for(&self, legacy: &str) -> Option<String> {
        self.point_by_key.get(&normalize_search(legacy)).cloned()
    }
}

fn is_placeholder(line: &str) -> bool {
    let line = line.trim();
    let lower = line.to_lowercase();
    line == "Этот текст можно"
        || line == "поменять в файле"
        || (lower.starts_with(SETTINGS_PREFIX) && lower.ends_with(".txt"))
}

fn is_untouched(file: &str, lines: &[String]) -> bool {
    LEGACY_DEFAULT_FILES
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(file))
        .is_some_and(|(_, default)| {
            let given: Vec<&str> = lines
                .iter()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty())
                .collect();
            given == *default
        })
}

/// Прочитанный файл списка: строки и признак «не менялся».
struct ListFile {
    lines: Vec<String>,
    untouched: bool,
}

fn read_list(folder: &LegacyFolder, file: &str, w: &mut Warnings) -> Option<ListFile> {
    let path = format!("{SETTINGS_PREFIX}{file}");
    let bytes = folder.text_files.get(&path)?;
    let (text, encoding) = decode_text(bytes);
    if encoding == TextEncoding::Utf8 {
        w.push(
            WarningCode::TextDecodedUtf8,
            "file",
            &path,
            "",
            "",
            "Файл не в Windows-1251, прочитан как UTF-8.",
        );
    }
    let lines = text_lines(&text);
    let untouched = is_untouched(file, &lines);
    Some(ListFile { lines, untouched })
}

/// Непустые строки без подсказок и повторов (повтор — после `normalize_search`).
fn clean_values(lines: &[String], path: &str, w: &mut Warnings) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut values = Vec::new();
    let mut skipped = 0;
    for line in lines {
        let value = line.trim();
        if value.is_empty() {
            continue;
        }
        if is_placeholder(value) {
            skipped += 1;
        } else if seen.insert(normalize_search(value)) {
            values.push(value.to_owned());
        }
    }
    if skipped > 0 {
        w.push(
            WarningCode::DictionaryPlaceholderSkipped,
            "file",
            path,
            "",
            &skipped.to_string(),
            "Строки-подсказки из стартового файла MyWork 2 пропущены.",
        );
    }
    values
}

fn default_values(kind: DictionaryKind) -> Vec<String> {
    DEFAULT_DICTIONARY
        .iter()
        .find(|(k, _)| *k == kind)
        .map(|(_, items)| items.iter().map(|s| (*s).to_owned()).collect())
        .unwrap_or_default()
}

fn convert_dictionaries(
    folder: &LegacyFolder,
    w: &mut Warnings,
) -> Vec<(DictionaryKind, Vec<String>)> {
    DEFAULT_DICTIONARY
        .iter()
        .map(|(kind, _)| {
            let file = DICTIONARY_FILES
                .iter()
                .find(|(k, _)| k == kind)
                .map(|(_, f)| *f);
            let values = match file.and_then(|f| read_list(folder, f, w).map(|l| (f, l))) {
                None => default_values(*kind),
                Some((f, list)) if list.untouched => {
                    w.push(
                        WarningCode::DictionaryDefaultKept,
                        "file",
                        &format!("{SETTINGS_PREFIX}{f}"),
                        "",
                        "",
                        "Файл не менялся, взяты стандартные значения MyWork 3.",
                    );
                    default_values(*kind)
                }
                Some((f, list)) => clean_values(&list.lines, &format!("{SETTINGS_PREFIX}{f}"), w),
            };
            (*kind, values)
        })
        .collect()
}

fn convert_points(db: &LegacyDb, folder: &LegacyFolder, w: &mut Warnings) -> Vec<String> {
    let mut points: Vec<String> = Vec::new();
    let mut seen = BTreeSet::new();
    let mut add = |value: &str, points: &mut Vec<String>| {
        let value = value.trim();
        if !value.is_empty() && seen.insert(normalize_search(value)) {
            points.push(value.to_owned());
        }
    };
    if let Some(list) = read_list(folder, "AdresSC.txt", w) {
        if list.untouched {
            w.push(
                WarningCode::DictionaryDefaultKept,
                "file",
                "settings/AdresSC.txt",
                "",
                "",
                "Файл не менялся, примеры адресов не перенесены.",
            );
        } else {
            for value in clean_values(&list.lines, "settings/AdresSC.txt", w) {
                add(&value, &mut points);
            }
        }
    }
    if let Some(table) = db.table("Catalog") {
        for row in &table.rows {
            add(&table.text(row, "AdressSC"), &mut points);
        }
    }
    points
}

fn convert_statuses(
    db: &LegacyDb,
    issued: &dyn Fn(i64) -> bool,
    w: &mut Warnings,
) -> Vec<NewStatus> {
    let mut statuses: Vec<NewStatus> = DEFAULT_STATUSES
        .iter()
        .map(|s| NewStatus {
            name: s.name.to_owned(),
            kind: s.kind,
            color: s.color.to_owned(),
        })
        .collect();
    let rename_from = normalize_search("Ждет ЗИП");
    // Нормализованное имя → (название, все ли заказы выданы), в порядке появления.
    let mut custom: Vec<(String, String, bool)> = Vec::new();
    let mut renamed_seen = BTreeSet::new();
    let Some(table) = db.table("Catalog") else {
        return statuses;
    };
    for row in &table.rows {
        let raw = table.text(row, "Status_remonta");
        let name = raw.trim();
        let name = if name.is_empty() { NO_STATUS } else { name };
        let key = normalize_search(name);
        if key == rename_from {
            if renamed_seen.insert(key) {
                w.push(
                    WarningCode::StatusRenamed,
                    "Catalog",
                    name,
                    "Status_remonta",
                    name,
                    "Статус «Ждет ЗИП» называется в MyWork 3 «Ждёт запчасть».",
                );
            }
            continue;
        }
        if statuses.iter().any(|s| normalize_search(&s.name) == key) {
            continue;
        }
        let order_issued = table.text(row, "id").trim().parse().is_ok_and(issued);
        match custom.iter_mut().find(|(k, _, _)| *k == key) {
            Some(entry) => entry.2 &= order_issued,
            None => custom.push((key, name.to_owned(), order_issued)),
        }
    }
    for (_, name, all_issued) in custom {
        w.push(
            WarningCode::StatusCustom,
            "Catalog",
            &name,
            "Status_remonta",
            &name,
            "В MyWork 3 добавлен статус из старой базы.",
        );
        statuses.push(NewStatus {
            name,
            kind: if all_issued {
                StatusKind::Issued
            } else {
                StatusKind::Open
            },
            color: if all_issued { "gray" } else { "blue" }.to_owned(),
        });
    }
    statuses
}

/// Собирает статусы, точки и справочники.
pub(crate) fn convert_catalogs(
    db: &LegacyDb,
    folder: &LegacyFolder,
    issued: &dyn Fn(i64) -> bool,
    w: &mut Warnings,
) -> Catalogs {
    let statuses = convert_statuses(db, issued, w);
    let service_points = convert_points(db, folder, w);
    let dictionary = convert_dictionaries(folder, w);
    let mut status_by_key: HashMap<String, String> = statuses
        .iter()
        .map(|s| (normalize_search(&s.name), s.name.clone()))
        .collect();
    status_by_key.insert(normalize_search("Ждет ЗИП"), "Ждёт запчасть".to_owned());
    let point_by_key = service_points
        .iter()
        .map(|p| (normalize_search(p), p.clone()))
        .collect();
    Catalogs {
        statuses,
        service_points,
        dictionary,
        status_by_key,
        point_by_key,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::FixtureVersion::V32;
    use crate::report::Warnings;
    use crate::test_support::prepared;
    use mw_domain::dictionary::{DEFAULT_DICTIONARY, DictionaryKind};
    use mw_domain::status::StatusKind;

    #[tokio::test]
    async fn statuses_points_and_dictionaries_follow_the_rules() {
        let (db, folder) = prepared(V32).await;
        let mut w = Warnings::default();
        let c = convert_catalogs(&db, &folder, &|id| [1, 3, 6].contains(&id), &mut w);
        assert_eq!(
            c.statuses
                .iter()
                .skip(8)
                .map(|s| (s.name.as_str(), s.kind))
                .collect::<Vec<_>>(),
            [
                ("Куплен", StatusKind::Issued),
                ("Отдан сервисному центру", StatusKind::Open),
                ("Без статуса", StatusKind::Open)
            ]
        );
        assert_eq!(c.status_for("Ждет ЗИП"), "Ждёт запчасть");
        assert_eq!(c.service_points, ["Мира, 5", "Ленина, 1", "ВОКЗАЛЬНАЯ"]);
        assert_eq!(c.point_for("ЛЕНИНА, 1").as_deref(), Some("Ленина, 1"));
        let dict = |k: DictionaryKind| {
            c.dictionary
                .iter()
                .find(|(kind, _)| *kind == k)
                .unwrap()
                .1
                .clone()
        };
        let defaults = |k: DictionaryKind| {
            DEFAULT_DICTIONARY
                .iter()
                .find(|(kind, _)| *kind == k)
                .unwrap()
                .1
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(dict(DictionaryKind::Brand), ["Samsung", "Apple"]);
        assert_eq!(dict(DictionaryKind::Master), ["ПЕТРОВ", "СИДОРОВ"]);
        assert_eq!(
            dict(DictionaryKind::DeviceType),
            defaults(DictionaryKind::DeviceType)
        );
        assert_eq!(
            dict(DictionaryKind::ClientSource),
            defaults(DictionaryKind::ClientSource)
        );
        assert_eq!(
            dict(DictionaryKind::Warranty),
            ["30 дней", "3 месяца", "Без гарантии"]
        );
        assert_eq!(c.dictionary.len(), DEFAULT_DICTIONARY.len());
        let info: Vec<_> = w.list().iter().map(|x| x.code.as_str()).collect();
        for code in [
            "status_renamed",
            "dictionary_placeholder_skipped",
            "dictionary_default_kept",
            "text_decoded_utf8",
        ] {
            assert!(info.contains(&code), "{code}");
        }
        assert_eq!(info.iter().filter(|c| **c == "status_custom").count(), 3);
    }

    #[test]
    fn untouched_file_ignores_blank_lines_and_edge_spaces() {
        let lines: Vec<String> = [
            "",
            " Мастер1 ",
            "",
            "Этот текст можно",
            "поменять в файле",
            "settings/masters.txt",
            "",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        assert!(is_untouched("masters.txt", &lines));
        let edited: Vec<String> = ["Мастер1", "Иванов"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        assert!(!is_untouched("masters.txt", &edited));
    }
}
