//! Синтетическая папка MyWork 2: собирается кодом, все имена и номера вымышлены.

use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
use sqlx::{AssertSqlSafe, Connection};

/// Токен СМС из `Config.ini` фикстуры.
pub const FIXTURE_SMS_TOKEN: &str = "fixture-sms-token-7f3a";

/// Версия схемы старой базы.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FixtureVersion {
    V24,
    V25,
    V26,
    V27,
    V29,
    V30,
    V31,
    V32,
}

impl FixtureVersion {
    pub const ALL: [FixtureVersion; 8] = [
        Self::V24,
        Self::V25,
        Self::V26,
        Self::V27,
        Self::V29,
        Self::V30,
        Self::V31,
        Self::V32,
    ];

    /// Разбирает `v24`…`v32` (регистр не важен).
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.to_lowercase();
        Self::ALL
            .into_iter()
            .find(|v| format!("{v:?}").to_lowercase() == text)
    }

    /// Число колонок `Catalog`.
    fn catalog_columns(self) -> usize {
        match self {
            Self::V24 => 24,
            Self::V25 => 25,
            Self::V26 => 26,
            Self::V27 => 27,
            Self::V29 => 29,
            Self::V30 => 30,
            Self::V31 => 31,
            Self::V32 => 32,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FixtureOptions {
    pub version: FixtureVersion,
    /// Сколько чистых заказов (номера от 100) добавить для проверки объёма.
    pub extra_orders: usize,
}

const CATALOG_BASE: &str = "CREATE TABLE IF NOT EXISTS Catalog (id INTEGER PRIMARY KEY AUTOINCREMENT, Data_priema TEXT, Data_vidachi TEXT, Data_predoplaty TEXT, surname TEXT,phone TEXT, AboutUs TEXT, WhatRemont TEXT, brand TEXT, model TEXT, SerialNumber TEXT, sostoyanie TEXT, komplektonst TEXT, polomka TEXT,kommentarij  TEXT, predvaritelnaya_stoimost TEXT, Predoplata TEXT, Zatrati TEXT, okonchatelnaya_stoimost_remonta TEXT,Skidka TEXT, Status_remonta TEXT,master TEXT, vipolnenie_raboti TEXT,Garanty TEXT)";

const CATALOG_BASE_COLUMNS: [&str; 24] = [
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
];

/// Колонки, которые MyWork 2 добавляла `ALTER TABLE Catalog ADD COLUMN X` (без типа), в порядке добавления.
const CATALOG_ADDED_COLUMNS: [&str; 8] = [
    "wait_zakaz",
    "Adress",
    "Image_key",
    "AdressSC",
    "DeviceColour",
    "ClientId",
    "Barcode",
    "Deleted",
];

const CREATE_CLIENTS_MAP: &str = "CREATE TABLE IF NOT EXISTS ClientsMap (id INTEGER PRIMARY KEY AUTOINCREMENT, FIO TEXT, Phone TEXT, Adress Text, Primechanie TEXT, Blist TEXT ,date TEXT, aboutUs TEXT)";
const CREATE_STATES_MAP: &str = "CREATE TABLE IF NOT EXISTS StatesMap (id INTEGER PRIMARY KEY AUTOINCREMENT, clientId TEXT, State TEXT ,date TEXT)";
const CREATE_HISTORY: &str = "CREATE TABLE IF NOT EXISTS HistoryBD(id INTEGER PRIMARY KEY AUTOINCREMENT, WHO TEXT, WHAT TEXT, FULLWHAT TEXT, DATA TEXT, IDINCATALOG TEXT)";
const CREATE_STOCK: &str = "CREATE TABLE IF NOT EXISTS Stock (id INTEGER PRIMARY KEY AUTOINCREMENT, Naimenovanie TEXT, Kategoriya TEXT, Podkategoriya TEXT, Colour TEXT, Brand TEXT, Model TEXT,CountOf TEXT, Price TEXT, Napominanie TEXT, Photo TEXT, Primechanie TEXT, Photo2 TEXT, Photo3 TEXT)";
const CREATE_STOCK_MAP: &str = "CREATE TABLE IF NOT EXISTS StockMap (id INTEGER PRIMARY KEY AUTOINCREMENT, clientId TEXT,ZIPId TEXT ,countOfZIP TEXT, priceOfZIP TEXT)";
const CREATE_USERS: &str = "CREATE TABLE IF NOT EXISTS Users(id INTEGER PRIMARY KEY AUTOINCREMENT, type TEXT, name TEXT, id_gruppi_dostupa, user_pwd TEXT)";
const CREATE_GROUPS: &str = "CREATE TABLE IF NOT EXISTS GroupDostup(id INTEGER PRIMARY KEY AUTOINCREMENT, grName TEXT, delZapis TEXT, addZapis  TEXT, saveZapis TEXT, graf TEXT, sms TEXT, stock TEXT, clients TEXT, stockAdd TEXT, stockDel TEXT, stockEdit TEXT, clientAdd TEXT, clientDel TEXT, clientConcat TEXT, settings TEXT, dates TEXT, editDates TEXT)";

const GROUP_COLUMNS: &str = "grName, delZapis, addZapis, saveZapis, graf, sms, stock, clients, stockAdd, stockDel, stockEdit, clientAdd, clientDel, clientConcat, settings, dates, editDates";

/// Клиент из `ClientsMap`: (id, ФИО, телефон, примечание, чёрный список, дата, «откуда узнал»).
type ClientRow = (
    i64,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
);

const CLIENTS: [ClientRow; 9] = [
    (
        1,
        "ИВАНОВ ИВАН",
        "89211234567,89117654321",
        "",
        "0",
        "2014-11-02",
        "Интернет",
    ),
    (
        2,
        "СИДОРОВА АННА",
        "8921123456789117654321",
        "",
        "1",
        "05-03-2015 14:20",
        "интернет",
    ),
    (3, "КИМ ОЛЕГ", "89211234567жена", "постоянный", "-1", "", ""),
    (4, "ОРЛОВ", "-", "", "True", "07-03-2015 10:00", ""),
    (
        5,
        "ИВАНОВ ИВАН",
        "89211234567,89117654321",
        "",
        "x",
        "08-03-2015 10:00",
        "",
    ),
    (
        6,
        "ЗАЙЦЕВ",
        "89211234567/89117654321(раб)",
        "",
        "0",
        "09-03-2015 10:00",
        "",
    ),
    (7, "ГРОМОВ", "нет телефона", "", "0", "10-03-2015 10:00", ""),
    (
        8,
        "БЕЛОВ",
        "89211234567123456",
        "",
        "0",
        "11-03-2015 10:00",
        "",
    ),
    (9, "ЧЕРНОВ", "1234", "", "0", "12-03-2015 10:00", ""),
];

/// Заказы эталона: номер и значения, отличающиеся от значений по умолчанию.
const ORDERS: [(i64, &[(&str, &str)]); 11] = [
    (
        1,
        &[
            ("Data_priema", "2014-11-02"),
            ("Data_vidachi", "2014-11-05"),
            ("Status_remonta", "Выдан"),
            ("ClientId", "1"),
            ("okonchatelnaya_stoimost_remonta", "1500"),
            ("Garanty", "30 дней"),
            ("Barcode", "036000291452"),
            // Справочники из заказов: бренд только в заказах тремя записями двух написаний («Nokia»
            // чаще). Поля-списки — как их пишет MyWork 2: неисправности — «пункт/ »
            // (`AddPosition.cs`), выполненные работы — пункт и перевод строки (`Editor.cs`),
            // комплектность — пункты подряд без разделителя («Аппарат» + «АКБ»).
            ("brand", "nokia"),
            ("polomka", "Нет изображения/ Шумит/ "),
            ("vipolnenie_raboti", "Замена дисплея\r\nЧистка\r\n"),
            ("komplektonst", "АппаратАКБ"),
        ],
    ),
    (
        2,
        &[
            ("Data_priema", "05-03-2015 14:20"),
            ("Data_predoplaty", "05.03.2015"),
            ("Status_remonta", "Ждет ЗИП"),
            ("ClientId", "1"),
            ("predvaritelnaya_stoimost", "1 500"),
            ("Predoplata", "500"),
            ("wait_zakaz", "Заказать"),
            ("Garanty", "3 мес"),
            (
                "kommentarij",
                "Позвонить завтра\r\n###RA123456789RU посылка",
            ),
            ("Barcode", "123456789012"),
            ("brand", "Nokia"),
            // Затраты вместе с запчастями со склада (`StockMap` 1 и 2 — 2 500 и 1 200,50): после
            // выделения запчастей остаётся 299,50.
            ("Zatrati", "4 000"),
            // Точка в конце остаётся в тексте заказа; в справочнике — тот же пункт, что «Шумит/ » (№1).
            ("polomka", "Шумит."),
            ("vipolnenie_raboti", "Чистка\r\n"),
            // Состояние MyWork 2 пишет «пункт, ».
            ("sostoyanie", "Царапины, Потёртости, "),
            ("komplektonst", "Аппарат"),
        ],
    ),
    (
        3,
        &[
            ("Data_priema", "06-03-2015 09:05"),
            ("Data_vidachi", "07-03-2015 18:00"),
            ("Status_remonta", "Куплен"),
            ("ClientId", "2"),
            ("Image_key", "1"),
            ("okonchatelnaya_stoimost_remonta", "0"),
            ("Barcode", "123456789012"),
            ("brand", "Nokia"),
            ("polomka", "нет изображения/ "),
            // Гарантия одними цифрами — срок в днях: одно значение с «90 дней» (№6), написание — с
            // единицей; «0» (№4) — без букв и без срока: не значение.
            ("Garanty", "90"),
            ("sostoyanie", "царапины"),
            ("komplektonst", "АппаратЗарядное устройство"),
        ],
    ),
    (
        4,
        &[
            ("Data_priema", "вчера"),
            ("Status_remonta", "Готов"),
            ("ClientId", "2"),
            ("predvaritelnaya_stoimost", "abc"),
            ("Image_key", "True"),
            ("Barcode", ""),
            // Бренд из `brands.txt` другим регистром: в справочнике — написание файла.
            ("brand", "SAMSUNG"),
            // Пункт дважды в одном заказе — это один заказ: в справочник не идёт.
            ("polomka", "Течёт/ течёт/ "),
            ("Garanty", "0"),
            // Затраты меньше запчастей со склада (`StockMap` 6 — 2 × 2 500): остаются как есть.
            ("Zatrati", "1000"),
        ],
    ),
    (
        5,
        &[
            ("Data_priema", "31-02-2015 10:00"),
            ("Status_remonta", "Отдан сервисному центру"),
            ("ClientId", "3"),
            ("Barcode", "12345"),
            ("brand", "SAMSUNG"),
            // Два написания одного мастера по одному заказу: имя — не заглавными (заказ №6).
            ("master", "МАСТЕР 6"),
            // Пункт есть в `neispravnost.txt`: одно значение с ним, место — среди значений файла.
            ("polomka", "Не включается/ "),
            // Затраты с запчастью, позиции которой на складе уже нет (`StockMap` 3 — 300).
            ("Zatrati", "500"),
        ],
    ),
    (
        6,
        &[
            ("Data_priema", "12.03.2015 15:04:05"),
            ("Data_vidachi", "13/03/2015"),
            ("Status_remonta", "Выдан"),
            ("ClientId", "3"),
            ("okonchatelnaya_stoimost_remonta", "15000000"),
            ("AdressSC", "ВОКЗАЛЬНАЯ"),
            // Два написания бренда по одному заказу: в справочнике — первое, заглавными (№7 — «Lg»).
            ("brand", "LG"),
            ("Garanty", "90 дней"),
        ],
    ),
    (
        7,
        &[
            ("Data_priema", "14-03-2015 10-30"),
            ("Status_remonta", "Выдан"),
            ("ClientId", "4"),
            // Мастер — пользователь MyWork 2 «Олег» (в другом регистре).
            ("master", "олег"),
            ("brand", "Lg"),
        ],
    ),
    (
        8,
        &[
            ("Data_priema", "15-03-2015 11:00"),
            ("Status_remonta", ""),
            ("ClientId", "5"),
            ("Deleted", "1"),
        ],
    ),
    (
        9,
        &[
            ("Data_priema", "16-03-2015 12:00"),
            ("Status_remonta", "Диагностика"),
            ("ClientId", ""),
            // Мастер без букв — не мастер: заказ без мастера, текст остаётся.
            ("master", "-"),
        ],
    ),
    (
        10,
        &[
            ("Data_priema", "17-03-2015 12:00"),
            ("Status_remonta", "Диагностика"),
            ("ClientId", "77"),
            ("surname", "ПЕТРОВ"),
            ("phone", "89110000000"),
            ("master", "-"),
        ],
    ),
    (
        12,
        &[
            ("Data_priema", "03/20/2015 9:15:00 PM"),
            ("Status_remonta", "Принят в работу"),
            ("ClientId", "6"),
            ("wait_zakaz", "что-то"),
            ("AdressSC", "ЛЕНИНА, 1"),
            // Лишние пробелы внутри имени мастера схлопываются: «Мастер 12».
            ("master", "Мастер   12"),
            // Бренд в одном заказе — не в справочник, но он есть в `brands.txt`: в справочнике из файла.
            ("brand", "apple"),
        ],
    ),
];

const STATES: [(i64, &str, &str, &str); 8] = [
    (
        1,
        "2",
        "05-03-2015 14-20",
        "Установлен статус\r\nДиагностика",
    ),
    (2, "2", "Ждет ЗИП", "06-03-2015 10:00"),
    (
        3,
        "2",
        "06-03-2015 10-05",
        "Установлена галочка\r\nТребует заказа",
    ),
    (4, "3", "06-03-2015 09:10", "Назначен мастер \r\nПЕТРОВ"),
    (5, "4", "07-03-2015 11-00", "Установлен статус\r\nГотов"),
    (6, "999", "07-03-2015 12-00", "Установлен статус\r\nВыдан"),
    (7, "3", "мусор", "Сообщение отправлено\r\nВаш заказ готов"),
    (
        8,
        "3",
        "07-03-2015 18:00",
        "Снята галочка\r\nНужно позвонить",
    ),
];

const HISTORY: [(i64, &str, &str, &str, &str, &str); 5] = [
    (
        1,
        "ADMIN",
        "ДОБАВЛЕНИЕ НОВОЙ ЗАПИСИ",
        "",
        "02-11-2014 10:00",
        "1",
    ),
    (
        2,
        "ИРИНА АДМИНЫ",
        "СОХРАНЕНИЕ",
        "Бренд: SAMSUNG → APPLE",
        "05-03-2015 15:00",
        "2",
    ),
    (3, "ADMIN", "УДАЛЕНИЕ", "", "16-03-2015 12:00", "8"),
    (
        4,
        "ADMIN",
        "ПЕЧАТЬ АКТА ВЫДАЧИ",
        "",
        "31-13-2015 10:00",
        "3",
    ),
    (5, "ADMIN", "СОХРАНЕНИЕ", "", "01-04-2015 10:00", "555"),
];

const CONFIG_INI: &str = "[PROGRAMM_SETTINGS]\r\nvaluta=руб.\r\nMpersent=40\r\ndaysDiagnostik=-5\r\nNessesaryCHB=Unchecked\r\ncolorDiagnostik=-16711936\r\ncolorCheckBox=Checked\r\n[CHECKBOX]\r\ngarantyDefault=Checked\r\nEveryDayBackup=Checked\r\n[SMSSEND]\r\nToken=@TOKEN@\r\n[ACTIVATION]\r\nWD-WCC123=deadbeef\r\n[WD-WCC123]\r\nAdressSCDefault=1\r\nMasterDefault=ПЕТРОВ\r\nBlistColor=-65536\r\nbackupPath=D:\\Backup\r\nsurname=150\r\n[S2Z9]\r\nMasterDefault=СИДОРОВ\r\n[ACTS]\r\nBarcodeH=40\r\nBarcodeW=120\r\n";

fn db_error(error: sqlx::Error) -> std::io::Error {
    std::io::Error::other(error)
}

/// Контрольная цифра UPC-A для 11 цифр.
fn upc_a(body: u64) -> String {
    let digits: Vec<u32> = format!("{body:011}")
        .chars()
        .filter_map(|c| c.to_digit(10))
        .collect();
    let sum: u32 = digits
        .iter()
        .enumerate()
        .map(|(i, d)| if i % 2 == 0 { d * 3 } else { *d })
        .sum();
    format!("{body:011}{}", (10 - sum % 10) % 10)
}

fn money_column(column: &str) -> bool {
    matches!(
        column,
        "predvaritelnaya_stoimost"
            | "Predoplata"
            | "Zatrati"
            | "okonchatelnaya_stoimost_remonta"
            | "Skidka"
    )
}

/// Колонки `Catalog` этой версии в порядке таблицы.
fn catalog_columns(version: FixtureVersion) -> Vec<&'static str> {
    let added = version.catalog_columns() - CATALOG_BASE_COLUMNS.len();
    CATALOG_BASE_COLUMNS
        .iter()
        .chain(CATALOG_ADDED_COLUMNS.iter().take(added))
        .copied()
        .collect()
}

fn catalog_row(version: FixtureVersion, id: i64, overrides: &[(&str, &str)]) -> Vec<String> {
    let find = |column: &str| {
        overrides
            .iter()
            .find(|(c, _)| *c == column)
            .map(|(_, v)| (*v).to_owned())
    };
    let client_id: i64 = find("ClientId").and_then(|v| v.parse().ok()).unwrap_or(0);
    let client = CLIENTS.iter().find(|c| c.0 == client_id);
    catalog_columns(version)
        .into_iter()
        .map(|column| {
            if column == "id" {
                return id.to_string();
            }
            if let Some(value) = find(column) {
                return value;
            }
            match column {
                // Каждое текстовое поле у каждого заказа своё: перепутанные поля поймает сверка «строка → поле».
                "WhatRemont" => format!("Устройство {id}"),
                "brand" => format!("Бренд {id}"),
                "model" => format!("Модель {id}"),
                "polomka" => format!("Поломка {id}"),
                "master" => format!("Мастер {id}"),
                "SerialNumber" => format!("SN-{id}"),
                "sostoyanie" => format!("Состояние {id}"),
                "komplektonst" => format!("Комплект {id}"),
                "DeviceColour" => format!("ЦВЕТ {id}"),
                "vipolnenie_raboti" => format!("Работа {id}"),
                "kommentarij" => format!("Комментарий {id}"),
                "Barcode" => upc_a(400_000 + id as u64),
                // До версии 30 клиент хранился прямо в заказе.
                "surname" if version < FixtureVersion::V30 => match (id, client) {
                    (8, _) => "иванов иван ".to_owned(),
                    (_, Some(c)) => c.1.to_owned(),
                    _ => String::new(),
                },
                "phone" if version < FixtureVersion::V30 => match (id, client) {
                    (8, _) => CLIENTS[0].2.to_owned(),
                    (_, Some(c)) => c.2.to_owned(),
                    _ => String::new(),
                },
                c if money_column(c) => "0".to_owned(),
                _ => String::new(),
            }
        })
        .collect()
}

async fn insert(
    conn: &mut sqlx::SqliteConnection,
    table: &str,
    columns: &[&str],
    values: &[String],
) -> std::io::Result<()> {
    let marks = vec!["?"; columns.len()].join(", ");
    let sql = format!(
        "INSERT INTO {table} ({}) VALUES ({marks})",
        columns.join(", ")
    );
    let mut query = sqlx::query(AssertSqlSafe(sql));
    for (column, value) in columns.iter().zip(values) {
        query = if *column == "id" {
            query.bind(value.parse::<i64>().unwrap_or(0))
        } else {
            query.bind(value.clone())
        };
    }
    query.execute(&mut *conn).await.map_err(db_error)?;
    Ok(())
}

async fn exec(conn: &mut sqlx::SqliteConnection, sql: &str) -> std::io::Result<()> {
    sqlx::query(AssertSqlSafe(sql.to_owned()))
        .execute(&mut *conn)
        .await
        .map_err(db_error)?;
    Ok(())
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| (*v).to_owned()).collect()
}

async fn write_database(path: &Path, options: &FixtureOptions) -> std::io::Result<()> {
    let connect = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Delete);
    let mut conn = sqlx::SqliteConnection::connect_with(&connect)
        .await
        .map_err(db_error)?;
    let result = fill_database(&mut conn, options).await;
    conn.close().await.map_err(db_error)?;
    result
}

async fn fill_database(
    conn: &mut sqlx::SqliteConnection,
    options: &FixtureOptions,
) -> std::io::Result<()> {
    let version = options.version;

    exec(conn, CATALOG_BASE).await?;
    for column in CATALOG_ADDED_COLUMNS
        .iter()
        .take(version.catalog_columns() - CATALOG_BASE_COLUMNS.len())
    {
        exec(conn, &format!("ALTER TABLE Catalog ADD COLUMN {column}")).await?;
    }
    if version >= FixtureVersion::V27 {
        exec(conn, CREATE_STATES_MAP).await?;
    }
    if version >= FixtureVersion::V29 {
        exec(conn, CREATE_STOCK).await?;
    }
    if version >= FixtureVersion::V30 {
        exec(conn, CREATE_CLIENTS_MAP).await?;
        exec(conn, CREATE_STOCK_MAP).await?;
    }
    if version >= FixtureVersion::V31 {
        exec(conn, CREATE_HISTORY).await?;
        exec(conn, CREATE_USERS).await?;
        exec(conn, CREATE_GROUPS).await?;
    }

    exec(conn, "BEGIN").await?;
    let columns = catalog_columns(version);
    for (id, overrides) in ORDERS {
        insert(
            conn,
            "Catalog",
            &columns,
            &catalog_row(version, id, overrides),
        )
        .await?;
    }
    for i in 0..options.extra_orders {
        let id = 100 + i as i64;
        let client = format!("{}", 100 + i);
        let mut overrides = vec![
            ("Data_priema", "01-01-2016 10:00"),
            ("Status_remonta", "Диагностика"),
        ];
        if version >= FixtureVersion::V30 {
            overrides.push(("ClientId", &client));
        }
        let fio = format!("КЛИЕНТ {i}");
        let phone = format!("8900{i:07}");
        if version < FixtureVersion::V30 {
            overrides.push(("surname", &fio));
            overrides.push(("phone", &phone));
        }
        insert(
            conn,
            "Catalog",
            &columns,
            &catalog_row(version, id, &overrides),
        )
        .await?;
        if version >= FixtureVersion::V30 {
            insert(
                conn,
                "ClientsMap",
                &[
                    "id",
                    "FIO",
                    "Phone",
                    "Adress",
                    "Primechanie",
                    "Blist",
                    "date",
                    "aboutUs",
                ],
                &[
                    client,
                    fio,
                    phone,
                    String::new(),
                    String::new(),
                    "0".into(),
                    "01-01-2016 10:00".into(),
                    String::new(),
                ],
            )
            .await?;
        }
        if version >= FixtureVersion::V27 {
            insert(
                conn,
                "StatesMap",
                &["id", "clientId", "State", "date"],
                &[
                    (100 + i).to_string(),
                    id.to_string(),
                    "01-01-2016 10:00".into(),
                    "Установлен статус\r\nДиагностика".into(),
                ],
            )
            .await?;
        }
    }
    if version >= FixtureVersion::V30 {
        for (id, fio, phone, note, blist, date, about) in CLIENTS {
            let values = vec![
                id.to_string(),
                fio.into(),
                phone.into(),
                format!("Адрес {id}"),
                if note.is_empty() {
                    format!("Заметка {id}")
                } else {
                    note.into()
                },
                blist.into(),
                date.into(),
                if about.is_empty() {
                    format!("Источник {id}")
                } else {
                    about.into()
                },
            ];
            insert(
                conn,
                "ClientsMap",
                &[
                    "id",
                    "FIO",
                    "Phone",
                    "Adress",
                    "Primechanie",
                    "Blist",
                    "date",
                    "aboutUs",
                ],
                &values,
            )
            .await?;
        }
    }
    if version == FixtureVersion::V32 {
        // Испорченный текст: один байт, не являющийся UTF-8 (серийный номер заказа 7).
        exec(
            conn,
            "UPDATE Catalog SET SerialNumber = CAST(X'C0' AS TEXT) WHERE id = 7",
        )
        .await?;
    }
    if version >= FixtureVersion::V27 {
        for (id, order, state, date) in STATES {
            insert(
                conn,
                "StatesMap",
                &["id", "clientId", "State", "date"],
                &strings(&[&id.to_string(), order, state, date]),
            )
            .await?;
        }
    }
    if version >= FixtureVersion::V29 {
        let columns = [
            "id",
            "Naimenovanie",
            "Kategoriya",
            "Podkategoriya",
            "Colour",
            "Brand",
            "Model",
            "CountOf",
            "Price",
            "Napominanie",
            "Photo",
            "Primechanie",
            "Photo2",
            "Photo3",
        ];
        insert(
            conn,
            "Stock",
            &columns,
            &strings(&[
                "1",
                "Дисплей",
                "Запчасти",
                "Дисплеи",
                "Чёрный",
                "SAMSUNG",
                "A52",
                "3",
                "2500",
                "1",
                "settings\\Stock\\Photos\\abc123.jpg",
                "",
                "settings\\Stock\\Photos\\missing.jpg",
                "",
            ]),
        )
        .await?;
        insert(
            conn,
            "Stock",
            &columns,
            &strings(&[
                "2",
                "Аккумулятор",
                "",
                "",
                "",
                "",
                "",
                "два",
                "",
                "",
                "",
                "",
                "",
                "",
            ]),
        )
        .await?;
        // Как пишет MyWork 2: наименование собрано кодом из полей (двойной пробел — пустой цвет),
        // всё заглавными; цена с пробелом, копейками и «р.»; три фото.
        insert(
            conn,
            "Stock",
            &columns,
            &strings(&[
                "3",
                "ТЕЛЕФОН АККУМУЛЯТОР XIAOMI REDMI 9  (ОРИГИНАЛ)",
                "ТЕЛЕФОН",
                "АККУМУЛЯТОР",
                "",
                "XIAOMI",
                "REDMI 9",
                "2",
                "1 200,50 р.",
                "5",
                "settings\\Stock\\Photos\\p1.jpg",
                "ОРИГИНАЛ",
                "settings\\Stock\\Photos\\p2.png",
                "settings\\Stock\\Photos\\p3.jpeg",
            ]),
        )
        .await?;
        // Количество и порог — десятичной записью `decimal` («3.0», «3,00»), цена — не число.
        insert(
            conn,
            "Stock",
            &columns,
            &strings(&[
                "4",
                "ТЕЛЕФОН ДИСПЛЕЙ",
                "ТЕЛЕФОН",
                "ДИСПЛЕЙ",
                "",
                "",
                "",
                "3.0",
                "abc",
                "3,00",
                "",
                "",
                "",
                "",
            ]),
        )
        .await?;
    }
    if version >= FixtureVersion::V30 {
        // Списания со склада: `clientId` — номер заказа, `ZIPId` — позиция, цена — на момент
        // списания. Заказ 2 — две позиции; заказ 5 — позиции 99 нет; заказа 404 нет; заказ 3 —
        // количество не число; заказ 4 — запчастей больше затрат.
        for row in [
            ["1", "2", "1", "1", "2500"],
            ["2", "2", "3", "1", "1200,50"],
            ["3", "5", "99", "1", "300"],
            ["4", "404", "1", "1", "2500"],
            ["5", "3", "2", "два", "800"],
            ["6", "4", "1", "2", "2500"],
        ] {
            insert(
                conn,
                "StockMap",
                &["id", "clientId", "ZIPId", "countOfZIP", "priceOfZIP"],
                &strings(&row),
            )
            .await?;
        }
    }
    if version >= FixtureVersion::V31 {
        for (id, who, what, full, date, order) in HISTORY {
            insert(
                conn,
                "HistoryBD",
                &["id", "WHO", "WHAT", "FULLWHAT", "DATA", "IDINCATALOG"],
                &strings(&[&id.to_string(), who, what, full, date, order]),
            )
            .await?;
        }
        write_access(conn, version).await?;
    }
    // «Удалённые» последние строки: счётчики AUTOINCREMENT выше максимального id.
    exec(
        conn,
        "UPDATE sqlite_sequence SET seq = MAX(seq, 15) WHERE name = 'Catalog'",
    )
    .await?;
    if version >= FixtureVersion::V30 {
        exec(
            conn,
            "UPDATE sqlite_sequence SET seq = MAX(seq, 12) WHERE name = 'ClientsMap'",
        )
        .await?;
    }
    exec(conn, "COMMIT").await?;
    Ok(())
}

/// Группы доступа и пользователи; пароли — SHA-1 по правилам MyWork 2.
async fn write_access(
    conn: &mut sqlx::SqliteConnection,
    version: FixtureVersion,
) -> std::io::Result<()> {
    let columns: Vec<&str> = GROUP_COLUMNS.split(", ").collect();
    let group = |name: &str, flags: [&str; 16]| {
        let mut values = vec![name.to_owned()];
        values.extend(flags.iter().map(|f| (*f).to_owned()));
        values
    };
    let sha1 = mw_core::legacy_sha1_hex;
    let (groups, users): (Vec<Vec<String>>, Vec<[String; 4]>) = if version == FixtureVersion::V31 {
        let mut flags = ["0"; 16];
        flags[2] = "1";
        (
            vec![group("Мастера", flags)],
            vec![["Мастера".into(), "Мастер".into(), "1".into(), sha1("1111")]],
        )
    } else {
        let mut master = ["0"; 16];
        master[0] = "False";
        master[2] = "True";
        master[6] = "1";
        (
            vec![
                group("Админы", ["1"; 16]),
                group("Мастера", master),
                group("Администратор", ["0"; 16]),
            ],
            vec![
                ["Админы".into(), "Ирина".into(), "1".into(), sha1("1234")],
                ["Мастера".into(), "ирина".into(), "2".into(), sha1("пароль")],
                [
                    "Мастера".into(),
                    "Пётр".into(),
                    "9".into(),
                    sha1("qwerty").to_uppercase(),
                ],
                ["Мастера".into(), "Олег".into(), "2".into(), "zzz".into()],
            ],
        )
    };
    for (i, values) in groups.iter().enumerate() {
        let mut all = vec![(i + 1).to_string()];
        all.extend(values.iter().cloned());
        let mut names = vec!["id"];
        names.extend(&columns);
        insert(conn, "GroupDostup", &names, &all).await?;
    }
    for (i, user) in users.iter().enumerate() {
        let mut all = vec![(i + 1).to_string()];
        all.extend(user.iter().cloned());
        insert(
            conn,
            "Users",
            &["id", "type", "name", "id_gruppi_dostupa", "user_pwd"],
            &all,
        )
        .await?;
    }
    Ok(())
}

/// Синтетический акт приёма, каким его сохраняет Word: `<o:p>`, метка, разбитая тегами.
const INTAKE_TEMPLATE: &str = "<html><body><p>Акт N <b>NUMZAKAZ</b><b>PRINT</b><o:p></o:p></p>\r\n<p>Клиент: FIOPRINT</p>\r\n<p>PRICEFULLPRINT</p>\r\n<p>Гарантия на чистку — 14 дней</p></body></html>";

fn cp1251(text: &str) -> Vec<u8> {
    encoding_rs::WINDOWS_1251.encode(text).0.into_owned()
}

fn put(dir: &Path, relative: &str, bytes: &[u8]) -> std::io::Result<()> {
    let path = dir.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes)
}

/// Испорченный склад (`add_stock_oddities`):
/// - в примечании позиции 2 — байт не UTF-8, номер заказа списания 3 — «5» с символом NUL;
/// - у `Stock` — колонка, которой MyWork 2 не создаёт (`Garantiya`, заполнена у позиции 3), у
///   `StockMap` — тоже (`Mesto`, у списания 1 в ней байт не UTF-8);
/// - списание 7 в заказ 6 с пустым `ZIPId` (и пустой ценой);
/// - счётчик `Stock` в `sqlite_sequence` (20) больше наибольшего номера позиции (4): позиции 5–20
///   удалены ещё в MyWork 2;
/// - фото позиции 4 — имя без точки (`p4jpeg`: MyWork 2 брала последние четыре знака имени за
///   расширение), файл лежит в папке фото.
const STOCK_ODDITIES: [&str; 9] = [
    "UPDATE Stock SET Primechanie = CAST(X'C0' AS TEXT) WHERE id = 2",
    "UPDATE StockMap SET clientId = '5' || char(0) WHERE id = 3",
    "ALTER TABLE Stock ADD COLUMN Garantiya",
    "UPDATE Stock SET Garantiya = '6 мес.' WHERE id = 3",
    "INSERT INTO StockMap (id, clientId, ZIPId, countOfZIP, priceOfZIP) VALUES (7, '6', '', '1', '')",
    "ALTER TABLE StockMap ADD COLUMN Mesto",
    "UPDATE StockMap SET Mesto = CAST(X'C0' AS TEXT) WHERE id = 1",
    "UPDATE sqlite_sequence SET seq = 20 WHERE name = 'Stock'",
    "UPDATE Stock SET Photo = 'settings\\Stock\\Photos\\p4jpeg' WHERE id = 4",
];

/// Файл фото без точки в имени (`STOCK_ODDITIES`, позиция 4).
const STOCK_PHOTO_WITHOUT_DOT: &str = "settings/Stock/Photos/p4jpeg";

/// Портит склад готовой папки V30 и новее (`STOCK_ODDITIES`): ячейки, которые разбор склада не может
/// прочитать как есть, неизвестные колонки, пустой номер позиции у списания, счётчик выше
/// наибольшего номера и фото без точки в имени — для тестов переноса склада из прошлого импорта.
/// Общая фикстура не меняется: на её числа опираются другие тесты.
pub async fn add_stock_oddities(dir: &Path) -> std::io::Result<()> {
    let connect = SqliteConnectOptions::new()
        .filename(dir.join("baza.sqlite"))
        .journal_mode(SqliteJournalMode::Delete);
    let mut conn = sqlx::SqliteConnection::connect_with(&connect)
        .await
        .map_err(db_error)?;
    let mut result = Ok(());
    for sql in STOCK_ODDITIES {
        result = exec(&mut conn, sql).await;
        if result.is_err() {
            break;
        }
    }
    conn.close().await.map_err(db_error)?;
    result?;
    put(dir, STOCK_PHOTO_WITHOUT_DOT, b"stock-p4jpeg")
}

/// Собирает папку старой программы: `baza.sqlite`, `Config.ini`, `settings/…`, `ClientFiles/…`.
pub async fn write_fixture(dir: &Path, options: &FixtureOptions) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let db = dir.join("baza.sqlite");
    if db.exists() {
        std::fs::remove_file(&db)?;
    }
    write_database(&db, options).await?;

    put(
        dir,
        "Config.ini",
        &cp1251(&CONFIG_INI.replace("@TOKEN@", FIXTURE_SMS_TOKEN)),
    )?;
    put(
        dir,
        "settings/AdresSC.txt",
        &cp1251("Мира, 5\r\nЛенина, 1\r\n"),
    )?;
    put(
        dir,
        "settings/brands.txt",
        "\u{feff}Samsung\nApple\nsamsung\n".as_bytes(),
    )?;
    put(
        dir,
        "settings/masters.txt",
        &cp1251(
            "ПЕТРОВ\r\nСИДОРОВ\r\n--\r\nЭтот текст можно\r\nпоменять в файле\r\nsettings/masters.txt\r\n",
        ),
    )?;
    put(
        dir,
        "settings/ustrojstvo.txt",
        &cp1251(
            "Ноутбук\r\nТелефон\r\nПатифон\r\nЭтот текст можно\r\nпоменять в файле\r\nsettings/ustrojstvo.txt\r\n",
        ),
    )?;
    put(
        dir,
        "settings/neispravnost.txt",
        &cp1251("Не включается\r\nНет изображения\r\n"),
    )?;
    put(
        dir,
        "settings/Garanty.txt",
        "30 дней\n3 месяца\nБез гарантии\n".as_bytes(),
    )?;
    for (name, text) in [
        ("FirmName.txt", "СЕРВИС-ПЛЮС"),
        ("Phone.txt", "8 (800) 000-00-00"),
        (
            "DannieOFirme.txt",
            "Режим работы: 10–19<br>\r\nг. Город, ул. Примерная, 1",
        ),
        ("URDannie.txt", "ИП Примеров П.П., ИНН 000000000000"),
        (
            "DogovorTextPriem.txt",
            "Строка договора 1<br>\r\nСтрока договора 2",
        ),
        ("DogovorTextVidacha.txt", "Условия выдачи"),
    ] {
        put(dir, &format!("settings/Akts/{name}"), &cp1251(text))?;
    }
    put(dir, "settings/Stock/brands.txt", &cp1251("SAMSUNG"))?;
    // Списки склада (MyWork 2 их не создаёт, только читает): типы запчастей — справочник «Типы
    // запчастей»; строки остальных — в конец справочников, если их там нет («ПЛАНШЕТ», «ЗОЛОТОЙ»
    // уже есть другим регистром).
    put(
        dir,
        "settings/Stock/TypeOvZIP.txt",
        &cp1251("АККУМУЛЯТОР\r\nДИСПЛЕЙ\r\n"),
    )?;
    put(
        dir,
        "settings/Stock/ustrojstvo.txt",
        &cp1251("ПЛАНШЕТ\r\nЭЛЕКТРОСАМОКАТ\r\n"),
    )?;
    put(
        dir,
        "settings/Stock/DeviceColour.txt",
        &cp1251("ЗОЛОТОЙ\r\nБРОНЗОВЫЙ\r\n"),
    )?;
    put(dir, "Shablon_Priem.htm", &cp1251("<html>Акт</html>"))?;
    put(dir, "Shablon_Act_Priema.htm", &cp1251(INTAKE_TEMPLATE))?;
    if options.version >= FixtureVersion::V29 {
        put(dir, "settings/Stock/Photos/abc123.jpg", b"stock-photo")?;
        for name in ["p1.jpg", "p2.png", "p3.jpeg"] {
            put(
                dir,
                &format!("settings/Stock/Photos/{name}"),
                format!("stock-{name}").as_bytes(),
            )?;
        }
    }
    if options.version >= FixtureVersion::V30 {
        put(dir, "ClientFiles/1/1/photo.jpg", b"photo-1")?;
        put(dir, "ClientFiles/1/1/docs/act.pdf", b"%PDF-fixture")?;
        put(dir, "ClientFiles/2/3/video.mp4", b"video-3")?;
        put(dir, "ClientFiles/2/404/lost.jpg", b"lost")?;
        put(dir, "ClientFiles/2/note.txt", b"note")?;
    }
    Ok(())
}
