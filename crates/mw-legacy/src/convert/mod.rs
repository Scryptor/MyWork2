//! Преобразование данных MyWork 2 в части пакета импорта.

pub(crate) mod access;
pub(crate) mod catalogs;
pub(crate) mod clients;
pub(crate) mod dictionaries;
pub(crate) mod history;
pub(crate) mod orders;
pub(crate) mod settings;

use std::collections::BTreeSet;
use std::path::PathBuf;

use chrono::NaiveDateTime;
use mw_core::{ImportBundle, ImportedLegacyRow, ImportedOrder, LegacyTarget};
use serde_json::json;

use crate::folder::LegacyFolder;
use crate::report::{WarningCode, Warnings};
use crate::source::{FULL_CATALOG_COLUMNS, LegacyDb, LegacyValue, SchemaSummary};

/// Файл, который нужно скопировать в `attachments`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlannedFile {
    /// Путь относительно корня старой папки.
    pub source: PathBuf,
    /// Путь относительно `attachments`.
    pub dest: PathBuf,
    /// `ClientFiles` или `Stock.Photo`.
    pub legacy_source: &'static str,
    pub legacy_key: String,
    pub target: Option<LegacyTarget>,
}

/// Результат преобразования: пакет, предупреждения, файлы для копирования, сводка схемы.
pub(crate) struct Conversion {
    pub bundle: ImportBundle,
    pub warnings: Warnings,
    pub files: Vec<PlannedFile>,
    pub schema: SchemaSummary,
}

const KNOWN_TABLES: [&str; 8] = [
    "Catalog",
    "ClientsMap",
    "StatesMap",
    "HistoryBD",
    "GroupDostup",
    "Users",
    "Stock",
    "StockMap",
];

/// Строка таблицы целиком: ключ — `id`, иначе номер строки.
fn table_rows(
    db: &LegacyDb,
    name: &str,
    by_row: bool,
    target: impl Fn(&str) -> Option<LegacyTarget>,
) -> Vec<ImportedLegacyRow> {
    let Some(table) = db.table(name) else {
        return Vec::new();
    };
    table
        .rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let id = table.text(row, "id");
            let key = if by_row || id.trim().is_empty() {
                format!("строка {}", i + 1)
            } else {
                id.trim().to_owned()
            };
            ImportedLegacyRow {
                source: table.name.clone(),
                target: target(&key),
                source_key: key,
                raw: table.row_json(row),
            }
        })
        .collect()
}

/// Колонки, которые знает конвертер, по таблицам (`Stock` и `StockMap` сохраняются целиком).
fn known_columns(table: &str) -> Option<&'static [&'static str]> {
    Some(match table.to_lowercase().as_str() {
        "catalog" => &FULL_CATALOG_COLUMNS,
        "clientsmap" => &[
            "id",
            "FIO",
            "Phone",
            "Adress",
            "Primechanie",
            "Blist",
            "date",
            "aboutUs",
        ],
        "statesmap" => &["id", "clientId", "State", "date"],
        "historybd" => &["id", "WHO", "WHAT", "FULLWHAT", "DATA", "IDINCATALOG"],
        "users" => &["id", "type", "name", "id_gruppi_dostupa", "user_pwd"],
        "groupdostup" => &[
            "id",
            "grName",
            "delZapis",
            "addZapis",
            "saveZapis",
            "graf",
            "sms",
            "stock",
            "clients",
            "stockAdd",
            "stockDel",
            "stockEdit",
            "clientAdd",
            "clientDel",
            "clientConcat",
            "settings",
            "dates",
            "editDates",
        ],
        _ => return None,
    })
}

fn warn_unknown_columns(db: &LegacyDb, w: &mut Warnings) {
    for table in &db.tables {
        let Some(known) = known_columns(&table.name) else {
            continue;
        };
        for (i, column) in table.columns.iter().enumerate() {
            if known.iter().any(|k| k.eq_ignore_ascii_case(column)) {
                continue;
            }
            let filled = table
                .rows
                .iter()
                .filter(|r| r.get(i).is_some_and(|v| !v.as_text().trim().is_empty()))
                .count();
            w.push(
                WarningCode::UnknownColumn,
                &table.name,
                column,
                column,
                &filled.to_string(),
                "Неизвестная колонка: значения сохранены только в исходной строке (legacy_rows).",
            );
        }
    }
}

fn warn_skipped_entries(folder: &LegacyFolder, w: &mut Warnings) {
    for entry in &folder.skipped {
        w.push(
            WarningCode::FileUnreadable,
            entry.source,
            &entry.path,
            "",
            "",
            "Ссылка или особый файл не перенесён.",
        );
    }
}

/// Таблицы, ячейки которых попадают в поля нового заказа, клиента, истории, пользователей и ролей.
const TEXT_CHECKED_TABLES: [&str; 6] = [
    "Catalog",
    "ClientsMap",
    "StatesMap",
    "HistoryBD",
    "Users",
    "GroupDostup",
];

/// По одному предупреждению на испорченную ячейку (не UTF-8, двоичная) или ячейку с символом NUL; значение не печатается.
fn warn_text_cells(db: &LegacyDb, w: &mut Warnings) {
    for name in TEXT_CHECKED_TABLES {
        let Some(table) = db.table(name) else {
            continue;
        };
        for (n, row) in table.rows.iter().enumerate() {
            let id = table.text(row, "id");
            let key = if id.trim().is_empty() {
                format!("строка {}", n + 1)
            } else {
                id.trim().to_owned()
            };
            for (column, value) in table.columns.iter().zip(row) {
                let message = match value {
                    LegacyValue::InvalidText(_) | LegacyValue::Blob(_) => {
                        "Текст не является корректным UTF-8: нечитаемые символы заменены, исходные байты сохранены в legacy_rows."
                    }
                    LegacyValue::Text(s) if s.contains('\0') => {
                        "В тексте есть символ NUL: он удалён, исходное значение сохранено в legacy_rows."
                    }
                    _ => continue,
                };
                w.push(
                    WarningCode::TextInvalidUtf8,
                    &table.name,
                    &key,
                    column,
                    "",
                    message,
                );
            }
        }
    }
}

fn legacy_rows(
    db: &LegacyDb,
    orders: &[ImportedOrder],
    client_rows: Vec<ImportedLegacyRow>,
    w: &mut Warnings,
) -> Vec<ImportedLegacyRow> {
    let order_ids: BTreeSet<i64> = orders.iter().map(|o| o.id).collect();
    let mut rows = table_rows(db, "Catalog", false, |key| {
        key.parse()
            .ok()
            .filter(|id| order_ids.contains(id))
            .map(LegacyTarget::Order)
    });
    rows.extend(client_rows);
    for name in ["Stock", "StockMap"] {
        rows.extend(table_rows(db, name, false, |_| None));
    }
    if let Some(stock) = db.table("Stock").filter(|t| !t.rows.is_empty()) {
        w.push(
            WarningCode::StockPreserved,
            "Stock",
            "",
            "",
            &stock.rows.len().to_string(),
            "Склад сохранён в исходном виде до появления модуля склада.",
        );
    }
    for table in db
        .tables
        .iter()
        .filter(|t| !KNOWN_TABLES.iter().any(|k| k.eq_ignore_ascii_case(&t.name)))
    {
        w.push(
            WarningCode::UnknownTable,
            &table.name,
            "",
            "",
            &table.rows.len().to_string(),
            "Неизвестная таблица сохранена в исходном виде.",
        );
        rows.extend(table_rows(db, &table.name, true, |_| None));
    }
    for (name, seq) in &db.sequences {
        rows.push(ImportedLegacyRow {
            source: "sqlite_sequence".to_owned(),
            source_key: name.clone(),
            target: None,
            raw: json!({ "name": name, "seq": seq }),
        });
    }
    rows
}

fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// Путь несуществующего фото: только простое имя файла внутри папки фото, иначе заведомо несуществующая заглушка.
fn missing_photo_source(name: &str) -> PathBuf {
    let plain = !name.is_empty() && name != "." && name != ".." && !name.contains(':');
    let leaf = if plain { name } else { "<missing>" };
    PathBuf::from(format!("settings/Stock/Photos/{leaf}"))
}

fn relative_to_root(folder: &LegacyFolder, found: PathBuf) -> PathBuf {
    found
        .strip_prefix(&folder.root)
        .map(PathBuf::from)
        .unwrap_or(found)
}

fn client_file_plan(
    folder: &LegacyFolder,
    order_ids: &BTreeSet<i64>,
    w: &mut Warnings,
) -> Vec<PlannedFile> {
    folder
        .client_files
        .iter()
        .map(|path| {
            let parts: Vec<&str> = path.split('/').collect();
            let order = parts.get(1).and_then(|p| p.parse::<i64>().ok());
            let order = order.filter(|id| parts.len() > 2 && order_ids.contains(id));
            let dest = match order {
                Some(id) => format!("orders/{id}/{}", parts[2..].join("/")),
                None => {
                    w.push(
                        WarningCode::ClientFilesOrphan,
                        "file",
                        path,
                        "",
                        "",
                        "Файл без заказа перенесён в attachments/legacy/ClientFiles.",
                    );
                    format!("legacy/ClientFiles/{path}")
                }
            };
            let wanted = format!("ClientFiles/{path}");
            let source = folder
                .find_file(&wanted)
                .map_or_else(|| PathBuf::from(&wanted), |f| relative_to_root(folder, f));
            PlannedFile {
                source,
                dest: PathBuf::from(dest),
                legacy_source: "ClientFiles",
                legacy_key: path.clone(),
                target: order.map(LegacyTarget::Order),
            }
        })
        .collect()
}

fn stock_photo_plan(db: &LegacyDb, folder: &LegacyFolder) -> Vec<PlannedFile> {
    let Some(table) = db.table("Stock") else {
        return Vec::new();
    };
    let photo_columns: Vec<&String> = table
        .columns
        .iter()
        .filter(|c| c.to_lowercase().starts_with("photo"))
        .collect();
    let mut files = Vec::new();
    let mut used = BTreeSet::new();
    for row in &table.rows {
        let id = table.text(row, "id");
        for column in &photo_columns {
            let raw = table.text(row, column);
            let raw = raw.trim();
            if raw.is_empty() {
                continue;
            }
            let name = file_name(raw);
            let found = folder
                .find_file(raw)
                .or_else(|| folder.find_file(&format!("settings/Stock/Photos/{name}")));
            let source = found.map_or_else(
                || missing_photo_source(name),
                |f| relative_to_root(folder, f),
            );
            let mut dest = format!("stock/{}/{name}", id.trim());
            if !used.insert(dest.clone()) {
                dest = format!("stock/{}/{column}_{name}", id.trim());
                used.insert(dest.clone());
            }
            files.push(PlannedFile {
                source,
                dest: PathBuf::from(dest),
                legacy_source: "Stock.Photo",
                legacy_key: format!("{}/{column}", id.trim()),
                target: None,
            });
        }
    }
    files
}

/// Собирает пакет импорта из прочитанной базы и папки; `now` — время импорта для подстановок.
pub(crate) fn convert(db: &LegacyDb, folder: &LegacyFolder, now: NaiveDateTime) -> Conversion {
    let mut w = Warnings::default();
    let hints = history::history_hints(db);
    let issued = orders::issued_ids(db);
    let catalogs = catalogs::convert_catalogs(db, folder, &|id| issued.contains(&id), &mut w);
    let mut stage = clients::convert_clients(db, now, &mut w);
    let converted = orders::convert_orders(db, &catalogs, &stage.order_client, &hints, now, &mut w);
    clients::fill_created_at(&mut stage, &converted);
    catalogs::warn_junk_masters(&converted, &mut w);
    let dictionary =
        dictionaries::build_dictionaries(&catalogs.dictionary, &converted, &stage.clients, &mut w);
    let history = history::convert_history(db, &converted, now, &mut w);
    warn_unknown_columns(db, &mut w);
    warn_text_cells(db, &mut w);
    warn_skipped_entries(folder, &mut w);

    let (settings, setting_rows) = settings::convert_settings(folder, &catalogs, &mut w);
    let (roles, users, access_rows) = access::convert_access(db, &mut w);
    let mut rows = legacy_rows(db, &converted, std::mem::take(&mut stage.rows), &mut w);
    rows.extend(setting_rows);
    rows.extend(access_rows);

    let order_ids: BTreeSet<i64> = converted.iter().map(|o| o.id).collect();
    let mut files = client_file_plan(folder, &order_ids, &mut w);
    files.extend(stock_photo_plan(db, folder));

    let sequence = db
        .sequences
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("Catalog"))
        .map_or(0, |(_, v)| *v);
    let next_order_id = sequence.max(order_ids.last().copied().unwrap_or(0)) + 1;
    w.push(
        WarningCode::NextNumber,
        "Catalog",
        "Catalog",
        "",
        &next_order_id.to_string(),
        "Следующий заказ получит этот номер.",
    );
    let bundle = ImportBundle {
        source_path: folder.root.display().to_string(),
        statuses: catalogs.statuses,
        service_points: catalogs.service_points,
        dictionary,
        masters: catalogs.masters,
        roles,
        users,
        settings,
        clients: stage.clients,
        orders: converted,
        history,
        legacy_rows: rows,
        next_order_id,
        next_client_id: stage.next_client_id,
        report: json!({}),
    };
    Conversion {
        bundle,
        warnings: w,
        files,
        schema: db.schema(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use crate::fixture::FixtureVersion::{V25, V32};
    use crate::report::{Severity, WarningCode};
    use crate::test_support::{client, conversion_of, dt, order, rub};
    use mw_domain::barcode::order_barcode;
    use mw_domain::money::Money;
    use serde_json::json;

    #[tokio::test]
    async fn v32_bundle_matches_the_fixture() {
        let c = conversion_of(V32).await;
        let b = &c.bundle;
        assert_eq!(
            (
                b.orders.len(),
                b.clients.len(),
                b.next_order_id,
                b.next_client_id
            ),
            (11, 11, 16, 15)
        );
        let o2 = order(b, 2);
        assert_eq!(
            (o2.received_at, o2.prepaid_at, o2.status.as_str()),
            (
                dt("2015-03-05 14:20:00"),
                Some(dt("2015-03-05 00:00:00")),
                "Ждёт запчасть"
            )
        );
        assert_eq!(
            (
                o2.estimate_from,
                o2.estimate_to,
                o2.final_price,
                o2.prepayment
            ),
            (Some(rub(1500)), Some(rub(1500)), None, rub(500))
        );
        assert_eq!((o2.need_parts, o2.warranty.as_str()), (true, "3 мес"));
        assert_eq!(
            o2.tracking,
            [("RA123456789RU".to_owned(), "посылка".to_owned())]
        );
        assert!(o2.internal_comment.ends_with("###RA123456789RU посылка"));
        let o3 = order(b, 3);
        assert_eq!(
            (o3.status.as_str(), o3.final_price, o3.need_call),
            ("Куплен", Some(Money::ZERO), true)
        );
        let o4 = order(b, 4);
        assert_eq!(
            (o4.received_at, o4.estimate_from, o4.barcode.as_str()),
            (
                dt("2015-03-07 11:00:00"),
                None,
                order_barcode(4).unwrap().as_str()
            )
        );
        assert_eq!(order(b, 5).received_at, dt("2015-03-07 11:00:00"));
        assert_eq!(order(b, 6).issued_at, Some(dt("2015-03-13 00:00:00")));
        assert_eq!(order(b, 8).deleted_at, Some(dt("2015-03-16 12:00:00")));
        assert_eq!(order(b, 8).status, "Без статуса");
        assert_eq!(
            (order(b, 9).client_id, client(b, 13).full_name.as_str()),
            (13, "Клиент не указан")
        );
        assert_eq!(
            (order(b, 10).client_id, client(b, 14).full_name.as_str()),
            (14, "ПЕТРОВ")
        );
        assert_eq!(order(b, 12).service_point.as_deref(), Some("Ленина, 1"));
        assert_eq!(order(b, 6).service_point.as_deref(), Some("ВОКЗАЛЬНАЯ"));
        let ph = |id| {
            client(b, id)
                .phones
                .iter()
                .map(|p| (p.phone.clone(), p.note.clone()))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            ph(1),
            [
                ("89211234567".into(), String::new()),
                ("89117654321".into(), String::new())
            ]
        );
        assert_eq!(ph(3), [("89211234567".into(), "жена".to_owned())]);
        assert_eq!(
            ph(8),
            [
                ("89211234567".into(), String::new()),
                ("123456".into(), String::new())
            ]
        );
        assert_eq!(client(b, 7).note, "Заметка 7\nТелефон: нет телефона");
        assert!(client(b, 2).blacklisted && client(b, 4).blacklisted && !client(b, 5).blacklisted);
        let h2: Vec<_> = b
            .history
            .iter()
            .filter(|h| h.order_id == Some(2))
            .map(|h| (h.action.as_str(), h.summary.as_str()))
            .collect();
        assert_eq!(
            h2,
            [
                ("legacy.status", "Установлен статус «Диагностика»"),
                ("legacy.history", "СОХРАНЕНИЕ"),
                ("legacy.status", "Установлен статус «Ждет ЗИП»"),
                ("legacy.flag", "Установлена галочка «Требует заказа»")
            ]
        );
        assert!(b.history.windows(2).all(|w| w[0].at <= w[1].at));
        assert_eq!(b.history.len(), 13);
        let count = |src: &str| b.legacy_rows.iter().filter(|r| r.source == src).count();
        assert_eq!(
            (
                count("Catalog"),
                count("ClientsMap"),
                count("GroupDostup"),
                count("Stock"),
                count("StockMap")
            ),
            (11, 9, 3, 2, 1)
        );
        assert_eq!(c.files.len(), 7);
    }

    #[tokio::test]
    async fn invalid_utf8_cell_is_reported_without_its_content_and_kept_as_hex() {
        let c = conversion_of(V32).await;
        let found: Vec<_> = c
            .warnings
            .list()
            .iter()
            .filter(|w| w.code == WarningCode::TextInvalidUtf8)
            .collect();
        assert_eq!(found.len(), 1);
        let w = found[0];
        assert_eq!(
            (
                w.source.as_str(),
                w.key.as_str(),
                w.field.as_str(),
                w.value.as_str()
            ),
            ("Catalog", "7", "SerialNumber", "")
        );
        let raw = c
            .bundle
            .legacy_rows
            .iter()
            .find(|r| r.source == "Catalog" && r.source_key == "7")
            .unwrap();
        assert_eq!(raw.raw["SerialNumber"], json!({"hex": "c0"}));
        assert_eq!(order(&c.bundle, 7).serial, "\u{FFFD}");
    }

    #[tokio::test]
    async fn nul_is_stripped_and_reported_and_user_passwords_are_never_printed() {
        let c = crate::test_support::conversion_with_sql(
            V32,
            &[
                "UPDATE Catalog SET model = 'A' || char(0) || 'B' WHERE id = 2",
                "UPDATE Users SET user_pwd = CAST(X'C0FF' AS TEXT) WHERE id = 1",
            ],
        )
        .await;
        assert_eq!(order(&c.bundle, 2).model, "AB");
        let row = c
            .bundle
            .legacy_rows
            .iter()
            .find(|r| r.source == "Catalog" && r.source_key == "2")
            .unwrap();
        assert_eq!(row.raw["model"], json!({"hex": "410042"}));
        let found: Vec<_> = c
            .warnings
            .list()
            .iter()
            .filter(|w| w.code == WarningCode::TextInvalidUtf8)
            .map(|w| {
                (
                    w.source.as_str(),
                    w.key.as_str(),
                    w.field.as_str(),
                    w.value.as_str(),
                )
            })
            .collect();
        assert!(found.contains(&("Catalog", "2", "model", "")));
        assert!(found.contains(&("Users", "1", "user_pwd", "")));
    }

    /// Спецификация §6: у каждой строки старой базы все текстовые поля совпадают с ячейкой после
    /// обрезки пробелов; поля-списки — пункты ячейки в виде MyWork 3 «пункт, пункт».
    #[tokio::test]
    async fn every_text_field_matches_its_source_cell_for_every_version() {
        use crate::fixture::FixtureVersion;
        use crate::parse::lists::{ListField, list_text};
        // В фикстуре нет `komplektonst.txt`: выпадающий список MyWork 2 — стартовые строки.
        let offered = super::catalogs::legacy_default_lines("komplektonst.txt");
        for version in FixtureVersion::ALL {
            let (db, _folder) = crate::test_support::prepared(version).await;
            let c = conversion_of(version).await;
            let catalog = db.table("Catalog").unwrap();
            let mut checked = 0;
            for row in &catalog.rows {
                let id: i64 = catalog.text(row, "id").parse().unwrap();
                let o = order(&c.bundle, id);
                let cell = |name: &str| catalog.text(row, name).trim().to_owned();
                let got = [
                    ("WhatRemont", &o.device_type),
                    ("brand", &o.brand),
                    ("model", &o.model),
                    ("SerialNumber", &o.serial),
                    ("DeviceColour", &o.color),
                    ("kommentarij", &o.internal_comment),
                    ("master", &o.master),
                    ("Garanty", &o.warranty),
                ];
                for (column, value) in got {
                    assert_eq!(*value, cell(column), "{version:?} order {id} {column}");
                }
                // Поля-списки — в виде MyWork 3 «пункт, пункт».
                let lists = [
                    ("komplektonst", ListField::Completeness, &o.completeness),
                    ("sostoyanie", ListField::Condition, &o.device_condition),
                    ("polomka", ListField::Malfunction, &o.malfunction),
                    ("vipolnenie_raboti", ListField::WorkDone, &o.work_done),
                ];
                for (column, field, value) in lists {
                    let expected = list_text(field, &cell(column), &offered);
                    assert_eq!(*value, expected, "{version:?} order {id} {column}");
                }
                checked += 1;
            }
            assert_eq!(checked, 11, "{version:?}");
            if version >= FixtureVersion::V30 {
                let clients = db.table("ClientsMap").unwrap();
                for row in &clients.rows {
                    let id: i64 = clients.text(row, "id").parse().unwrap();
                    let k = client(&c.bundle, id);
                    let cell = |name: &str| clients.text(row, name).trim().to_owned();
                    assert_eq!(k.address, cell("Adress"), "{version:?} client {id}");
                    assert_eq!(k.source, cell("aboutUs"), "{version:?} client {id}");
                    assert!(
                        k.note.starts_with(&cell("Primechanie")),
                        "{version:?} client {id}"
                    );
                    assert!(!cell("Primechanie").is_empty());
                }
                assert_eq!(clients.rows.len(), 9);
            }
        }
    }

    #[tokio::test]
    async fn v32_warnings_are_exactly_expected() {
        let c = conversion_of(V32).await;
        let mut got: Vec<_> = c
            .warnings
            .list()
            .iter()
            .filter(|w| w.code.severity() == Severity::Warning)
            .map(|w| (w.code.as_str(), w.source.as_str(), w.key.as_str()))
            .collect();
        got.sort();
        assert_eq!(
            got,
            [
                ("amount_suspicious", "Catalog", "6"),
                ("barcode_duplicate", "Catalog", "123456789012"),
                ("barcode_invalid", "Catalog", "5"),
                ("client_missing", "Catalog", "10"),
                ("config_section_ignored", "Config.ini", "S2Z9"),
                ("date_month_first", "Catalog", "12"),
                ("date_unparsed", "Catalog", "4"),
                ("date_unparsed", "Catalog", "5"),
                ("date_unparsed", "HistoryBD", "4"),
                ("date_unparsed", "StatesMap", "7"),
                ("flag_unrecognized", "Catalog", "12"),
                ("flag_unrecognized", "ClientsMap", "5"),
                ("history_orphan", "HistoryBD", "5"),
                ("history_orphan", "StatesMap", "6"),
                ("issued_without_date", "Catalog", "7"),
                ("login_renamed", "Users", "2"),
                ("money_unparsed", "Catalog", "4"),
                ("order_without_client", "Catalog", "9"),
                ("phone_no_digits", "ClientsMap", "7"),
                ("phone_unusual", "ClientsMap", "9"),
                ("received_at_missing", "Catalog", "4"),
                ("received_at_missing", "Catalog", "5"),
                ("status_empty", "Catalog", "8"),
                ("text_invalid_utf8", "Catalog", "7"),
                ("user_group_missing", "Users", "3"),
                ("user_password_invalid", "Users", "4"),
            ]
        );
        let info = |code: &str| {
            c.warnings
                .list()
                .iter()
                .filter(|w| w.code.as_str() == code)
                .count()
        };
        assert_eq!(
            (
                info("phone_split"),
                info("client_duplicates"),
                info("barcode_generated"),
                info("tracking_found"),
                info("stock_preserved"),
                info("next_number")
            ),
            (6, 1, 1, 1, 1, 1)
        );
    }

    #[tokio::test]
    async fn old_schema_builds_clients_from_orders() {
        let c = conversion_of(V25).await;
        let b = &c.bundle;
        assert_eq!(
            (b.clients.len(), b.next_client_id, b.history.len()),
            (7, 8, 0)
        );
        assert_eq!(order(b, 8).client_id, order(b, 1).client_id);
        assert_eq!(
            client(b, order(b, 9).client_id).full_name,
            "Клиент не указан"
        );
        assert!(b.orders.iter().all(|o| !o.need_call));
    }

    use crate::test_support::conversion_with_sql;

    fn warned(c: &super::Conversion, code: &str, source: &str, key: &str) -> Option<String> {
        c.warnings
            .list()
            .iter()
            .find(|w| w.code.as_str() == code && w.source == source && w.key == key)
            .map(|w| w.value.clone())
    }

    #[tokio::test]
    async fn junk_phone_orders_get_separate_placeholder_clients() {
        let c = conversion_with_sql(
            V32,
            &[
                "UPDATE Catalog SET ClientId = '', surname = '', phone = '-' WHERE id = 3",
                "UPDATE Catalog SET ClientId = '', surname = '', phone = 'нет телефона' WHERE id = 4",
            ],
        )
        .await;
        let b = &c.bundle;
        let (a, d) = (order(b, 3).client_id, order(b, 4).client_id);
        assert_ne!(a, d);
        assert_eq!(client(b, a).full_name, "Клиент не указан");
        assert_eq!(client(b, d).full_name, "Клиент не указан");
        assert_eq!(client(b, d).note, "Телефон: нет телефона");
        assert!(warned(&c, "order_without_client", "Catalog", "3").is_some());
        assert!(warned(&c, "order_without_client", "Catalog", "4").is_some());
    }

    #[tokio::test]
    async fn unknown_columns_are_reported_and_kept_in_the_raw_row() {
        let c = conversion_with_sql(
            V32,
            &[
                "ALTER TABLE Catalog ADD COLUMN Extra",
                "UPDATE Catalog SET Extra = 'x' WHERE id = 2",
            ],
        )
        .await;
        assert_eq!(
            warned(&c, "unknown_column", "Catalog", "Extra").as_deref(),
            Some("1")
        );
        assert_eq!(
            c.warnings
                .list()
                .iter()
                .filter(|w| w.code.as_str() == "unknown_column")
                .count(),
            1
        );
        let row = c
            .bundle
            .legacy_rows
            .iter()
            .find(|r| r.source == "Catalog" && r.source_key == "2")
            .unwrap();
        assert_eq!(row.raw["Extra"], "x");
    }

    #[tokio::test]
    async fn missing_stock_photos_never_leave_the_folder() {
        let c = conversion_with_sql(
            V32,
            &["UPDATE Stock SET Photo = '..\\..\\secret.jpg', Photo2 = 'C:\\x\\y.jpg', Photo3 = '..' WHERE id = 1"],
        )
        .await;
        let photos: Vec<_> = c
            .files
            .iter()
            .filter(|f| f.legacy_source == "Stock.Photo")
            .collect();
        assert_eq!(photos.len(), 3);
        for f in photos {
            let text = f.source.to_string_lossy().into_owned();
            assert!(!text.split('/').any(|s| s == ".."), "{text}");
            assert!(
                !text.starts_with('/') && !text.contains(':') && !text.contains('\\'),
                "{text}"
            );
            assert!(text.starts_with("settings/Stock/Photos/"), "{text}");
        }
    }

    #[tokio::test]
    async fn unparseable_final_price_stays_empty_even_when_issued() {
        let c = conversion_with_sql(
            V32,
            &["UPDATE Catalog SET okonchatelnaya_stoimost_remonta = 'abc' WHERE id = 1"],
        )
        .await;
        assert_eq!(order(&c.bundle, 1).final_price, None);
        assert!(warned(&c, "money_unparsed", "Catalog", "1").is_some());
    }

    #[tokio::test]
    async fn merged_restored_client_takes_first_non_empty_address() {
        let c = conversion_with_sql(
            crate::fixture::FixtureVersion::V26,
            &["UPDATE Catalog SET Adress = 'Мира, 1' WHERE id = 8"],
        )
        .await;
        let b = &c.bundle;
        assert_eq!(order(b, 8).client_id, order(b, 1).client_id);
        assert_eq!(client(b, order(b, 1).client_id).address, "Мира, 1");
    }

    #[tokio::test]
    async fn unknown_table_rows_have_unique_keys_and_duplicates_list_client_ids() {
        let c = conversion_with_sql(
            V32,
            &[
                "CREATE TABLE Junk (id TEXT, v TEXT)",
                "INSERT INTO Junk VALUES (' 1', 'a'), ('1', 'b'), ('1', 'c')",
                "UPDATE HistoryBD SET WHAT = 'Удаление' WHERE id = 3",
            ],
        )
        .await;
        let keys: Vec<_> = c
            .bundle
            .legacy_rows
            .iter()
            .filter(|r| r.source == "Junk")
            .map(|r| r.source_key.clone())
            .collect();
        assert_eq!(keys.len(), 3);
        assert_eq!(keys.iter().collect::<BTreeSet<_>>().len(), 3);
        assert_eq!(
            order(&c.bundle, 8).deleted_at,
            Some(dt("2015-03-16 12:00:00"))
        );
        assert_eq!(
            warned(&c, "client_duplicates", "ClientsMap", "").as_deref(),
            Some("1, 5")
        );
    }

    fn dictionary(c: &super::Conversion, kind: mw_domain::dictionary::DictionaryKind) -> Vec<&str> {
        c.bundle
            .dictionary
            .iter()
            .find(|(k, _)| *k == kind)
            .map(|(_, values)| values.iter().map(String::as_str).collect())
            .unwrap()
    }

    fn standard(kind: mw_domain::dictionary::DictionaryKind) -> Vec<&'static str> {
        mw_domain::dictionary::DEFAULT_DICTIONARY
            .iter()
            .find(|(k, _)| *k == kind)
            .map(|(_, values)| values.to_vec())
            .unwrap()
    }

    /// Справочники — из значений заказов (по числу заказов), затем значения только из файлов.
    #[tokio::test]
    async fn dictionaries_come_from_orders_then_from_files() {
        use mw_domain::dictionary::DictionaryKind::*;
        let c = conversion_of(V32).await;
        // «Nokia» только в заказах (nokia, Nokia, Nokia) — одно значение, частое написание, первым;
        // «SAMSUNG» в заказах — «Samsung» из файла; «LG»/«Lg» поровну — первое, заглавные не
        // мешают; «Apple» только в файле — в конце; удалённый заказ №8 не считается.
        assert_eq!(
            dictionary(&c, Brand),
            [
                "Nokia",
                "Samsung",
                "LG",
                "Бренд 9",
                "Бренд 10",
                "Бренд 12",
                "Apple"
            ]
        );
        // Пункт списка — хотя бы из двух заказов: «Нет изображения/ » (№1) и «нет изображения.»
        // (№3) — одно значение с написанием `neispravnost.txt`; «Шумит» (один заказ) и «Течёт»
        // (дважды в одном заказе) — нет; «Не включается/ » (№5) — то же значение, что в файле, без
        // повтора.
        assert_eq!(
            dictionary(&c, Malfunction),
            ["Нет изображения", "Не включается"]
        );
        // Выполненные работы — по строкам; состояние — через запятую.
        let mut work = vec!["Чистка"];
        work.extend(standard(WorkDone).into_iter().filter(|v| *v != "Чистка"));
        assert_eq!(dictionary(&c, WorkDone), work);
        let mut condition = vec!["Царапины"];
        condition.extend(standard(Condition).into_iter().filter(|v| *v != "Царапины"));
        assert_eq!(dictionary(&c, Condition), condition);
        // Комплектность, склеенная из строк списка MyWork 2: «Аппарат» — в трёх заказах.
        let mut completeness = vec!["Аппарат"];
        completeness.extend(standard(Completeness));
        assert_eq!(dictionary(&c, Completeness), completeness);
        // Гарантия: «90» и «90 дней» — один срок, написание с единицей, два заказа — первым;
        // «3 мес» — «3 месяца» из файла; «0» — не значение.
        assert_eq!(
            dictionary(&c, Warranty),
            ["90 дней", "30 дней", "3 месяца", "Без гарантии"]
        );
        // «Откуда узнали» — из клиентов: «Интернет»/«интернет» — одно значение со стандартным написанием.
        let mut sources = vec!["Интернет"];
        let numbered: Vec<String> = (3..=9).map(|id| format!("Источник {id}")).collect();
        sources.extend(numbered.iter().map(String::as_str));
        sources.extend(
            standard(ClientSource)
                .into_iter()
                .filter(|v| *v != "Интернет"),
        );
        assert_eq!(dictionary(&c, ClientSource), sources);
        let devices: Vec<String> = [1, 2, 3, 4, 5, 6, 7, 9, 10, 12]
            .iter()
            .map(|id| format!("Устройство {id}"))
            .collect();
        let mut device_types: Vec<&str> = devices.iter().map(String::as_str).collect();
        device_types.extend(standard(DeviceType));
        assert_eq!(dictionary(&c, DeviceType), device_types);
        assert!(dictionary(&c, Master).is_empty());

        let info: Vec<(&str, &str)> = c
            .warnings
            .list()
            .iter()
            .filter(|w| w.code.as_str() == "dictionary_from_orders")
            .map(|w| (w.key.as_str(), w.value.as_str()))
            .collect();
        assert_eq!(
            info,
            [
                ("Типы устройств", "из заказов — 10, стандартных — 8"),
                ("Бренды", "из заказов — 6, из файла — 1"),
                ("Цвета", "из заказов — 10, стандартных — 8"),
                (
                    "Комплектность",
                    "из заказов — 1, стандартных — 7, пунктов из одного заказа — 9"
                ),
                (
                    "Состояние",
                    "из заказов — 1, стандартных — 4, пунктов из одного заказа — 9"
                ),
                (
                    "Неисправности",
                    "из заказов — 1, из файла — 1, пунктов из одного заказа — 9"
                ),
                (
                    "Выполненные работы",
                    "из заказов — 1, стандартных — 5, пунктов из одного заказа — 9"
                ),
                ("Откуда узнали", "из клиентов — 8, стандартных — 4"),
                ("Гарантия", "из заказов — 3, из файла — 1"),
            ]
        );
    }

    /// Поля-списки MyWork 2 («пункт/ », строки, «пункт, », пункты подряд) — в виде MyWork 3
    /// «пункт, пункт»; исходный текст — в исходной строке заказа.
    #[tokio::test]
    async fn list_fields_take_the_form_of_mywork3() {
        let c = conversion_of(V32).await;
        let b = &c.bundle;
        let lists = |id| {
            let o = order(b, id);
            (
                o.malfunction.as_str(),
                o.work_done.as_str(),
                o.device_condition.as_str(),
                o.completeness.as_str(),
            )
        };
        assert_eq!(
            lists(1),
            (
                "Нет изображения, Шумит",
                "Замена дисплея, Чистка",
                "Состояние 1",
                "Аппарат, АКБ"
            )
        );
        assert_eq!(
            lists(2),
            ("Поломка 2", "Чистка", "Царапины, Потёртости", "Аппарат")
        );
        assert_eq!(
            lists(3),
            (
                "нет изображения",
                "Работа 3",
                "царапины",
                "Аппарат, Зарядное устройство"
            )
        );
        assert_eq!(order(b, 4).malfunction, "Течёт, течёт");
        let raw = b
            .legacy_rows
            .iter()
            .find(|r| r.source == "Catalog" && r.source_key == "1")
            .unwrap();
        assert_eq!(raw.raw["polomka"], "Нет изображения/ Шумит/ ");
        assert_eq!(raw.raw["vipolnenie_raboti"], "Замена дисплея\r\nЧистка\r\n");
        assert_eq!(raw.raw["komplektonst"], "АппаратАКБ");
    }

    /// Мастер по умолчанию без букв (`MasterDefault=?`) — не мастер, строка в отчёте.
    #[tokio::test]
    async fn a_default_master_without_letters_is_reported() {
        let c = crate::test_support::conversion_with_text(V32, "Config.ini", &|text| {
            text.replace("MasterDefault=ПЕТРОВ", "MasterDefault=?")
        })
        .await;
        assert_eq!(c.bundle.settings.default_master, None);
        let junk: Vec<_> = c
            .warnings
            .list()
            .iter()
            .filter(|w| w.code.as_str() == "master_name_junk")
            .map(|w| {
                (
                    w.source.as_str(),
                    w.key.as_str(),
                    w.field.as_str(),
                    w.value.as_str(),
                )
            })
            .collect();
        assert!(
            junk.contains(&(
                "Config.ini",
                "[WD-WCC123] MasterDefault",
                "MasterDefault",
                "?"
            )),
            "{junk:?}"
        );
    }

    /// Имя мастера без букв — не мастер: в отчёте одной строкой на написание (с числом заказов),
    /// строка файла мастеров без букв — тоже.
    #[tokio::test]
    async fn master_names_without_letters_are_reported_not_listed() {
        let c = conversion_of(V32).await;
        assert_eq!(c.bundle.masters, ["ПЕТРОВ", "СИДОРОВ"]);
        assert_eq!(order(&c.bundle, 9).master, "-");
        let junk: Vec<(&str, &str, &str, &str, &str)> = c
            .warnings
            .list()
            .iter()
            .filter(|w| w.code.as_str() == "master_name_junk")
            .map(|w| {
                (
                    w.source.as_str(),
                    w.key.as_str(),
                    w.field.as_str(),
                    w.value.as_str(),
                    w.message.as_str(),
                )
            })
            .collect();
        assert_eq!(
            junk,
            [
                (
                    "file",
                    "settings/masters.txt",
                    "",
                    "--",
                    "Строка без букв — не мастер."
                ),
                (
                    "Catalog",
                    "",
                    "master",
                    "-",
                    "Заказов: 2. В имени нет букв: мастер не назначен, текст в заказах остался."
                ),
            ]
        );
    }
}
