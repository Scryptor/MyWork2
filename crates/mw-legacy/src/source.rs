//! Чтение копии базы MyWork 2: все таблицы целиком, ячейки с классом хранения.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Map, Number, Value, json};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
use sqlx::{AssertSqlSafe, Connection, Row};

use crate::error::LegacyError;

/// Полный набор колонок `Catalog` (версия 32).
pub(crate) const FULL_CATALOG_COLUMNS: [&str; 32] = [
    "id",
    "Data_priema",
    "Data_vidachi",
    "Data_predoplaty",
    "surname",
    "phone",
    "AboutUs",
    "WhatRemont",
    "brand",
    "model",
    "SerialNumber",
    "sostoyanie",
    "komplektonst",
    "polomka",
    "kommentarij",
    "predvaritelnaya_stoimost",
    "Predoplata",
    "Zatrati",
    "okonchatelnaya_stoimost_remonta",
    "Skidka",
    "Status_remonta",
    "master",
    "vipolnenie_raboti",
    "Garanty",
    "wait_zakaz",
    "Adress",
    "Image_key",
    "AdressSC",
    "DeviceColour",
    "ClientId",
    "Barcode",
    "Deleted",
];

/// Значение ячейки с сохранённым классом хранения SQLite.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum LegacyValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    /// Текст, который не является корректным UTF-8.
    InvalidText(Vec<u8>),
    Blob(Vec<u8>),
}

impl LegacyValue {
    /// Текстовое представление: NULL — пусто, числа — десятичной записью, испорченный текст — с заменой символов.
    pub(crate) fn as_text(&self) -> String {
        match self {
            Self::Null => String::new(),
            Self::Integer(n) => n.to_string(),
            Self::Real(x) => x.to_string(),
            Self::Text(s) => s.replace('\0', ""),
            Self::InvalidText(b) | Self::Blob(b) => String::from_utf8_lossy(b).replace('\0', ""),
        }
    }

    pub(crate) fn to_json(&self) -> Value {
        match self {
            Self::Null => Value::Null,
            Self::Integer(n) => Value::from(*n),
            Self::Real(x) => Number::from_f64(*x).map_or(Value::Null, Value::Number),
            // PostgreSQL не принимает NUL в тексте и в jsonb: такие значения хранятся шестнадцатеричной записью.
            Self::Text(s) if s.contains('\0') => json!({ "hex": hex::encode(s.as_bytes()) }),
            Self::Text(s) => Value::String(s.clone()),
            Self::InvalidText(b) | Self::Blob(b) => json!({ "hex": hex::encode(b) }),
        }
    }

    /// Обратное `to_json` (значение из следа `legacy_rows`): `{"hex": …}` — текст с NUL, если байты —
    /// UTF-8 с NUL; `InvalidText`, если не UTF-8; иначе двоичное значение. Так `to_json` и `as_text`
    /// прочитанного дают то же, что у исходного. Чего `to_json` не выдаёт — текстом JSON.
    pub(crate) fn from_json(value: &Value) -> Self {
        match value {
            Value::Null => Self::Null,
            Value::String(s) => Self::Text(s.clone()),
            Value::Number(n) => match (n.as_i64(), n.as_f64()) {
                (Some(i), _) => Self::Integer(i),
                (None, Some(x)) => Self::Real(x),
                (None, None) => Self::Text(n.to_string()),
            },
            Value::Object(object) if object.len() == 1 => {
                let bytes = object
                    .get("hex")
                    .and_then(Value::as_str)
                    .and_then(|h| hex::decode(h).ok());
                match bytes {
                    Some(bytes) => match String::from_utf8(bytes) {
                        Ok(text) if text.contains('\0') => Self::Text(text),
                        Ok(text) => Self::Blob(text.into_bytes()),
                        Err(error) => Self::InvalidText(error.into_bytes()),
                    },
                    None => Self::Text(value.to_string()),
                }
            }
            other => Self::Text(other.to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct LegacyTable {
    pub name: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<LegacyValue>>,
}

impl LegacyTable {
    /// Номер колонки без учёта регистра (SQLite не различает регистр имён колонок).
    pub(crate) fn column(&self, name: &str) -> Option<usize> {
        let wanted = name.to_lowercase();
        self.columns.iter().position(|c| c.to_lowercase() == wanted)
    }

    /// Текст ячейки; нет колонки или строки — пусто.
    pub(crate) fn text(&self, row: &[LegacyValue], name: &str) -> String {
        self.column(name)
            .and_then(|i| row.get(i))
            .map(LegacyValue::as_text)
            .unwrap_or_default()
    }

    /// Строка как JSON-объект «колонка → значение».
    pub(crate) fn row_json(&self, row: &[LegacyValue]) -> Value {
        let mut object = Map::new();
        for (name, value) in self.columns.iter().zip(row) {
            object
                .entry(name.clone())
                .or_insert_with(|| value.to_json());
        }
        Value::Object(object)
    }

    /// Таблица из строк-следов (обратное `row_json`): колонки — ключи строк в порядке появления,
    /// нет ключа в строке — NULL. Не объект — строка без значений.
    pub(crate) fn from_json_rows(name: &str, rows: &[Value]) -> LegacyTable {
        let mut columns: Vec<String> = Vec::new();
        for object in rows.iter().filter_map(Value::as_object) {
            for key in object.keys() {
                if !columns.contains(key) {
                    columns.push(key.clone());
                }
            }
        }
        let rows = rows
            .iter()
            .map(|row| {
                columns
                    .iter()
                    .map(|c| row.get(c).map_or(LegacyValue::Null, LegacyValue::from_json))
                    .collect()
            })
            .collect();
        LegacyTable {
            name: name.to_owned(),
            columns,
            rows,
        }
    }
}

/// Краткая сводка по схеме старой базы.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SchemaSummary {
    pub catalog_columns: usize,
    /// Колонки `Catalog` из полного набора (32), которых в этой базе нет.
    pub missing_columns: Vec<String>,
    /// Таблицы и число строк.
    pub tables: Vec<(String, u64)>,
}

impl SchemaSummary {
    /// Версия базы для людей: «колонок в заказах — 32», при нехватке — со списком недостающих.
    pub fn version_text(&self) -> String {
        let mut out = format!("колонок в заказах — {}", self.catalog_columns);
        if !self.missing_columns.is_empty() {
            out.push_str(&format!(
                " (нет колонок: {})",
                self.missing_columns.join(", ")
            ));
        }
        out
    }
}

#[derive(Debug, Clone)]
pub(crate) struct LegacyDb {
    pub tables: Vec<LegacyTable>,
    /// `sqlite_sequence`: имя таблицы → последний выданный id.
    pub sequences: BTreeMap<String, i64>,
}

impl LegacyDb {
    pub(crate) fn table(&self, name: &str) -> Option<&LegacyTable> {
        let wanted = name.to_lowercase();
        self.tables.iter().find(|t| t.name.to_lowercase() == wanted)
    }

    pub(crate) fn schema(&self) -> SchemaSummary {
        let catalog = self.table("Catalog");
        SchemaSummary {
            catalog_columns: catalog.map_or(0, |t| t.columns.len()),
            missing_columns: FULL_CATALOG_COLUMNS
                .iter()
                .filter(|c| catalog.is_none_or(|t| t.column(c).is_none()))
                .map(|c| (*c).to_owned())
                .collect(),
            tables: self
                .tables
                .iter()
                .map(|t| (t.name.clone(), t.rows.len() as u64))
                .collect(),
        }
    }
}

fn sqlite_error(error: sqlx::Error) -> LegacyError {
    LegacyError::Sqlite(error.to_string())
}

fn quote(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

/// Читает копию базы; исходный файл и его журналы не меняются.
pub(crate) async fn read_database(db_file: &Path) -> Result<LegacyDb, LegacyError> {
    let io = |context: &str| {
        let context = format!("{context}: {}", db_file.display());
        move |source| LegacyError::Io { context, source }
    };
    let temp = tempfile::TempDir::new().map_err(io("Не удалось создать временную папку"))?;
    let file_name = db_file
        .file_name()
        .ok_or_else(|| LegacyError::SourceNotFound(db_file.to_path_buf()))?;
    let copy = temp.path().join(file_name);
    if !db_file.is_file() {
        return Err(LegacyError::SourceNotFound(db_file.to_path_buf()));
    }
    tokio::fs::copy(db_file, &copy)
        .await
        .map_err(io("Не удалось скопировать базу"))?;
    for suffix in ["-journal", "-wal", "-shm"] {
        let mut sibling = db_file.as_os_str().to_owned();
        sibling.push(suffix);
        let mut target = copy.as_os_str().to_owned();
        target.push(suffix);
        if Path::new(&sibling).is_file() {
            tokio::fs::copy(&sibling, &target)
                .await
                .map_err(io("Не удалось скопировать журнал базы"))?;
        }
    }

    let options = SqliteConnectOptions::new()
        .filename(&copy)
        .create_if_missing(false)
        .journal_mode(SqliteJournalMode::Delete);
    let mut conn = sqlx::SqliteConnection::connect_with(&options)
        .await
        .map_err(sqlite_error)?;
    let result = read_all(&mut conn).await;
    let _ = conn.close().await;
    drop(temp);
    result
}

async fn read_all(conn: &mut sqlx::SqliteConnection) -> Result<LegacyDb, LegacyError> {
    let names: Vec<String> = sqlx::query(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\' ORDER BY rowid",
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(sqlite_error)?
    .iter()
    .map(|r| r.try_get::<String, _>(0))
    .collect::<Result<_, _>>()
    .map_err(sqlite_error)?;

    let mut tables = Vec::new();
    for name in names {
        tables.push(read_table(conn, &name).await?);
    }
    if !tables
        .iter()
        .any(|t| t.name.eq_ignore_ascii_case("Catalog"))
    {
        return Err(LegacyError::NotMyWork2);
    }

    let mut sequences = BTreeMap::new();
    let has_sequence: i64 =
        sqlx::query("SELECT count(*) FROM sqlite_master WHERE name = 'sqlite_sequence'")
            .fetch_one(&mut *conn)
            .await
            .and_then(|r| r.try_get(0))
            .map_err(sqlite_error)?;
    if has_sequence > 0 {
        for row in sqlx::query("SELECT name, seq FROM sqlite_sequence")
            .fetch_all(&mut *conn)
            .await
            .map_err(sqlite_error)?
        {
            let name: String = row.try_get(0).map_err(sqlite_error)?;
            let seq: i64 = row.try_get(1).map_err(sqlite_error)?;
            sequences.insert(name, seq);
        }
    }
    Ok(LegacyDb { tables, sequences })
}

async fn read_table(
    conn: &mut sqlx::SqliteConnection,
    name: &str,
) -> Result<LegacyTable, LegacyError> {
    let columns: Vec<String> =
        sqlx::query(AssertSqlSafe(format!("PRAGMA table_info({})", quote(name))))
            .fetch_all(&mut *conn)
            .await
            .map_err(sqlite_error)?
            .iter()
            .map(|r| r.try_get::<String, _>(1))
            .collect::<Result<_, _>>()
            .map_err(sqlite_error)?;

    // На каждую ячейку: класс хранения, байты и (для REAL) точное число.
    let select: Vec<String> = columns
        .iter()
        .map(|c| {
            let c = quote(c);
            format!("typeof({c}), CAST({c} AS BLOB), CASE WHEN typeof({c}) = 'real' THEN {c} END")
        })
        .collect();
    let sql = format!(
        "SELECT {} FROM {} ORDER BY rowid",
        select.join(", "),
        quote(name)
    );
    let mut rows = Vec::new();
    for row in sqlx::query(AssertSqlSafe(sql))
        .fetch_all(&mut *conn)
        .await
        .map_err(sqlite_error)?
    {
        let mut cells = Vec::with_capacity(columns.len());
        for i in 0..columns.len() {
            let class: String = row.try_get(i * 3).map_err(sqlite_error)?;
            let bytes: Option<Vec<u8>> = row.try_get(i * 3 + 1).map_err(sqlite_error)?;
            let bytes = bytes.unwrap_or_default();
            cells.push(match class.as_str() {
                "integer" => {
                    LegacyValue::Integer(String::from_utf8_lossy(&bytes).parse().map_err(|_| {
                        LegacyError::Sqlite(format!("Неверное целое число в таблице {name}"))
                    })?)
                }
                "real" => {
                    LegacyValue::Real(row.try_get::<f64, _>(i * 3 + 2).map_err(sqlite_error)?)
                }
                "text" => match String::from_utf8(bytes) {
                    Ok(text) => LegacyValue::Text(text),
                    Err(error) => LegacyValue::InvalidText(error.into_bytes()),
                },
                "blob" => LegacyValue::Blob(bytes),
                _ => LegacyValue::Null,
            });
        }
        rows.push(cells);
    }
    Ok(LegacyTable {
        name: name.to_owned(),
        columns,
        rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::{FixtureOptions, FixtureVersion, write_fixture};
    use crate::folder::scan_folder;
    use crate::test_support::{fixture, hash_tree};
    use serde_json::json;

    #[tokio::test]
    async fn schema_is_detected_by_column_names_for_every_fixture_version() {
        for version in FixtureVersion::ALL {
            let dir = tempfile::tempdir().unwrap();
            write_fixture(
                dir.path(),
                &FixtureOptions {
                    version,
                    extra_orders: 0,
                },
            )
            .await
            .unwrap();
            let db = read_database(&dir.path().join("baza.sqlite"))
                .await
                .unwrap();
            let s = db.schema();
            let expected = match version {
                FixtureVersion::V24 => 24,
                FixtureVersion::V25 => 25,
                FixtureVersion::V26 => 26,
                FixtureVersion::V27 => 27,
                FixtureVersion::V29 => 29,
                FixtureVersion::V30 => 30,
                FixtureVersion::V31 => 31,
                FixtureVersion::V32 => 32,
            };
            assert_eq!(s.catalog_columns, expected, "{version:?}");
            assert_eq!(db.table("catalog").unwrap().rows.len(), 11, "{version:?}");
            assert_eq!(db.sequences.get("Catalog"), Some(&15), "{version:?}");
            if version == FixtureVersion::V26 {
                assert!(s.missing_columns.contains(&"Image_key".to_owned()));
                assert!(!s.missing_columns.contains(&"Adress".to_owned()));
            }
        }
    }

    #[tokio::test]
    async fn cells_keep_their_storage_class() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("baza.sqlite");
        let opts = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true);
        let mut conn = sqlx::SqliteConnection::connect_with(&opts).await.unwrap();
        for sql in [
            "CREATE TABLE Catalog (id INTEGER PRIMARY KEY AUTOINCREMENT, Data_priema, Predoplata, Zatrati, polomka, kommentarij)",
            "INSERT INTO Catalog VALUES (1, NULL, 1500, 2.5, X'D0', CAST(X'C0' AS TEXT))",
        ] {
            sqlx::query(sql).execute(&mut conn).await.unwrap();
        }
        conn.close().await.unwrap();
        let db = read_database(&path).await.unwrap();
        let table = db.table("Catalog").unwrap();
        let json = table.row_json(&table.rows[0]);
        assert_eq!(
            (&json["Data_priema"], &json["Predoplata"], &json["Zatrati"]),
            (&json!(null), &json!(1500), &json!(2.5))
        );
        assert_eq!(json["polomka"], json!({"hex": "d0"}));
        assert_eq!(json["kommentarij"], json!({"hex": "c0"}));
    }

    /// Строка, сохранённая следом (`row_json`), читается обратно: текст, числа, NULL и `{"hex": …}`
    /// (текст с NUL, не UTF-8, двоичное значение) дают те же значения, тот же текст и тот же JSON.
    #[test]
    fn json_rows_read_back() {
        let table = LegacyTable {
            name: "Stock".into(),
            columns: [
                "id",
                "Naimenovanie",
                "Price",
                "CountOf",
                "Primechanie",
                "Model",
                "Brand",
                "Photo",
            ]
            .map(str::to_owned)
            .to_vec(),
            rows: vec![
                vec![
                    LegacyValue::Integer(3),
                    LegacyValue::Text("ДИСПЛЕЙ  A52".into()),
                    LegacyValue::Real(1200.5),
                    LegacyValue::Real(3.0),
                    LegacyValue::Null,
                    LegacyValue::Text("A\0B".into()),
                    LegacyValue::InvalidText(vec![0xD0, 0x41]),
                    LegacyValue::Blob(b"photo".to_vec()),
                ],
                vec![
                    LegacyValue::Text(" 4 ".into()),
                    LegacyValue::Text(String::new()),
                    LegacyValue::Text("abc".into()),
                    LegacyValue::Integer(-1),
                    LegacyValue::Text("1500".into()),
                    LegacyValue::Null,
                    LegacyValue::Null,
                    LegacyValue::Null,
                ],
            ],
        };
        let raws: Vec<Value> = table.rows.iter().map(|r| table.row_json(r)).collect();
        // Как хранит база: текстом, и обратно.
        let stored: Vec<Value> = raws
            .iter()
            .map(|r| serde_json::from_str(&r.to_string()).unwrap())
            .collect();
        let back = LegacyTable::from_json_rows("Stock", &stored);
        assert_eq!(back.name, "Stock");
        let mut columns = back.columns.clone();
        columns.sort();
        let mut expected = table.columns.clone();
        expected.sort();
        assert_eq!(columns, expected);
        for (row, original) in back.rows.iter().zip(&table.rows) {
            for (column, value) in table.columns.iter().zip(original) {
                assert_eq!(
                    back.text(row, column),
                    value.as_text(),
                    "{column}: {value:?}"
                );
            }
        }
        assert_eq!(back.rows.len(), 2);
        let first = &back.rows[0];
        let cell = |row: &[LegacyValue], column: &str| row[back.column(column).unwrap()].clone();
        assert_eq!(cell(first, "id"), LegacyValue::Integer(3));
        assert_eq!(cell(first, "Price"), LegacyValue::Real(1200.5));
        assert_eq!(cell(first, "CountOf"), LegacyValue::Real(3.0));
        assert_eq!(cell(first, "Primechanie"), LegacyValue::Null);
        assert_eq!(cell(first, "Model"), LegacyValue::Text("A\0B".into()));
        assert_eq!(
            cell(first, "Brand"),
            LegacyValue::InvalidText(vec![0xD0, 0x41])
        );
        assert_eq!(cell(first, "Photo"), LegacyValue::Blob(b"photo".to_vec()));
        assert_eq!(cell(&back.rows[1], "id"), LegacyValue::Text(" 4 ".into()));
        // И снова в JSON — тот же след.
        let again: Vec<Value> = back.rows.iter().map(|r| back.row_json(r)).collect();
        assert_eq!(again, stored);
    }

    #[tokio::test]
    async fn source_folder_is_not_modified() {
        let dir = fixture(FixtureVersion::V32).await;
        std::fs::write(dir.path().join("baza.sqlite-journal"), b"").unwrap();
        let before = hash_tree(dir.path());
        let _db = read_database(&dir.path().join("baza.sqlite"))
            .await
            .unwrap();
        let _folder = scan_folder(dir.path()).unwrap();
        assert_eq!(hash_tree(dir.path()), before);
    }

    #[tokio::test]
    async fn not_a_mywork2_database() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.sqlite");
        let opts = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true);
        let mut conn = sqlx::SqliteConnection::connect_with(&opts).await.unwrap();
        sqlx::query("CREATE TABLE t (a)")
            .execute(&mut conn)
            .await
            .unwrap();
        conn.close().await.unwrap();
        assert!(matches!(
            read_database(&path).await,
            Err(LegacyError::NotMyWork2)
        ));
    }
}
