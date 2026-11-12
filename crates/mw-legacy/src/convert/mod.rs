//! Преобразование данных MyWork 2 в части пакета импорта.

pub(crate) mod access;
pub(crate) mod catalogs;
pub(crate) mod clients;
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
use crate::source::{LegacyDb, SchemaSummary};

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
            let key = if id.trim().is_empty() {
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

fn legacy_rows(
    db: &LegacyDb,
    orders: &[ImportedOrder],
    client_rows: Vec<ImportedLegacyRow>,
    w: &mut Warnings,
) -> Vec<ImportedLegacyRow> {
    let order_ids: BTreeSet<i64> = orders.iter().map(|o| o.id).collect();
    let mut rows = table_rows(db, "Catalog", |key| {
        key.parse()
            .ok()
            .filter(|id| order_ids.contains(id))
            .map(LegacyTarget::Order)
    });
    rows.extend(client_rows);
    for name in ["Stock", "StockMap"] {
        rows.extend(table_rows(db, name, |_| None));
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
        rows.extend(table_rows(db, &table.name, |_| None));
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
                || PathBuf::from(raw.replace('\\', "/")),
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
    let history = history::convert_history(db, &converted, now, &mut w);

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
        dictionary: catalogs.dictionary,
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
    use crate::fixture::FixtureVersion::{V25, V32};
    use crate::report::Severity;
    use crate::test_support::{client, conversion_of, dt, order, rub};
    use mw_domain::barcode::order_barcode;
    use mw_domain::money::Money;

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
        assert_eq!(client(b, 7).note, "Телефон: нет телефона");
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
}
