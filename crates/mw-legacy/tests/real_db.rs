//! Проверка импорта на настоящей папке MyWork 2 (запускает автор вручную).
//!
//! `MW_LEGACY_REAL_DIR=<папка с baza.sqlite> cargo test -p mw-legacy --test real_db -- --ignored --nocapture`
//!
//! Тест печатает только числа и фиксированные подписи — никаких значений из базы.
//! Исходная база открывается только на чтение; на PostgreSQL своя схема удаляется в конце.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use chrono::{TimeZone, Utc};
use mw_core::{Actor, Core, FixedClock};
use mw_domain::money::Money;
use mw_legacy::{ImportOptions, WarningCode, import_folder};
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};
use sqlx::Connection;
use sqlx::sqlite::{SqliteConnectOptions, SqliteConnection};

/// Ожидаемые числа, посчитанные прямо по исходной базе.
struct Expected {
    orders: u64,
    deleted: u64,
    need_call: u64,
    need_parts: u64,
    clients_map: u64,
    blacklisted: u64,
    history: u64,
    next_order_id: i64,
    /// Строк `Stock` и `StockMap`.
    stock_rows: u64,
    stock_map_rows: u64,
    /// `estimate_from`, `prepayment`, `final_price`, `extra_costs`, `discount`; затраты — как в
    /// MyWork 2, с запчастями склада.
    money: [i64; 5],
    /// Поля-списки: (колонка `orders`, номер заказа → текст ячейки без краёв).
    lists: Vec<(&'static str, HashMap<i64, String>)>,
}

/// Слова строки справочника в отчёте — подписи программы; другое слово было бы текстом базы.
const DICTIONARY_LINE_WORDS: &[&str] = &[
    "из",
    "заказов",
    "клиентов",
    "файла",
    "стандартных",
    "значений",
    "пунктов",
    "одного",
    "заказа",
    "у",
    "клиента",
    "с",
    "переписанным",
    "полем",
    "со",
    "склада",
];

/// Строка справочника печатается, только если название — справочник программы, а в числах нет
/// слов, кроме подписей.
fn dictionary_line_is_safe(key: &str, value: &str) -> bool {
    let known = mw_domain::dictionary::DEFAULT_DICTIONARY
        .iter()
        .any(|(kind, _)| kind.label_ru() == key);
    known
        && value
            .split(|c: char| !c.is_alphabetic())
            .filter(|w| !w.is_empty())
            .all(|w| DICTIONARY_LINE_WORDS.contains(&w))
}

/// Поля-списки: колонка `Catalog` → колонка `orders`.
const LIST_COLUMNS: [(&str, &str); 4] = [
    ("komplektonst", "completeness"),
    ("sostoyanie", "device_condition"),
    ("polomka", "malfunction"),
    ("vipolnenie_raboti", "work_done"),
];

fn find_db(dir: &Path) -> PathBuf {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("baza.sqlite"))
        })
        .expect("в папке нет baza.sqlite")
}

async fn table_columns(conn: &mut SqliteConnection, table: &str) -> Option<BTreeSet<String>> {
    let names: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT lower(name) FROM pragma_table_info('{table}')"
    )))
    .fetch_all(conn)
    .await
    .unwrap();
    (!names.is_empty()).then(|| names.into_iter().collect())
}

/// Значения столбца как текст; `None` для отсутствующей таблицы или столбца.
async fn texts(conn: &mut SqliteConnection, table: &str, column: &str) -> Vec<Option<String>> {
    match table_columns(conn, table).await {
        Some(cols) if cols.contains(&column.to_lowercase()) => {
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                "SELECT CAST(\"{column}\" AS TEXT) FROM \"{table}\""
            )))
            .fetch_all(conn)
            .await
            .unwrap()
        }
        _ => Vec::new(),
    }
}

async fn count_rows(conn: &mut SqliteConnection, table: &str) -> u64 {
    if table_columns(conn, table).await.is_none() {
        return 0;
    }
    let n: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT COUNT(*) FROM \"{table}\""
    )))
    .fetch_one(conn)
    .await
    .unwrap();
    n as u64
}

/// Значения «истина» старой базы (как в разборщике флагов; на реальной базе это «1» и «Заказать»).
const TRUE_VALUES: &[&str] = &["1", "true", "да", "yes", "checked", "заказать"];

fn count_text(values: &[Option<String>], accepted: &[&str]) -> u64 {
    values
        .iter()
        .flatten()
        .filter(|v| accepted.contains(&v.trim().to_lowercase().as_str()))
        .count() as u64
}

async fn expected(db_file: &Path) -> Expected {
    let options = SqliteConnectOptions::new()
        .filename(db_file)
        .read_only(true)
        .immutable(true);
    let mut conn = SqliteConnection::connect_with(&options).await.unwrap();
    let c = &mut conn;

    let deleted = texts(c, "Catalog", "Deleted").await;
    let image_key = texts(c, "Catalog", "Image_key").await;
    let wait = texts(c, "Catalog", "wait_zakaz").await;
    let blist = texts(c, "ClientsMap", "Blist").await;
    let ids: Vec<i64> = texts(c, "Catalog", "id")
        .await
        .into_iter()
        .flatten()
        .filter_map(|v| v.trim().parse().ok())
        .collect();
    let sequence: i64 = if table_columns(c, "sqlite_sequence").await.is_some() {
        sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(MAX(CAST(seq AS INTEGER)), 0) FROM sqlite_sequence WHERE lower(name) = 'catalog'",
        )
        .fetch_one(&mut *c)
        .await
        .unwrap()
    } else {
        0
    };

    let mut money = [0i64; 5];
    for (i, column) in [
        "predvaritelnaya_stoimost",
        "Predoplata",
        "okonchatelnaya_stoimost_remonta",
        "Zatrati",
        "Skidka",
    ]
    .into_iter()
    .enumerate()
    {
        money[i] = texts(c, "Catalog", column)
            .await
            .into_iter()
            .flatten()
            .filter_map(|v| Money::parse(v.replace('\u{a0}', " ").trim()).ok())
            .map(Money::kopecks)
            .sum();
    }

    let mut lists = Vec::new();
    for (legacy, column) in LIST_COLUMNS {
        let rows: Vec<(Option<String>, Option<String>)> = sqlx::query_as(sqlx::AssertSqlSafe(
            format!("SELECT CAST(id AS TEXT), CAST(\"{legacy}\" AS TEXT) FROM Catalog"),
        ))
        .fetch_all(&mut *c)
        .await
        .unwrap();
        let cells = rows
            .into_iter()
            .filter_map(|(id, text)| {
                Some((
                    id?.trim().parse().ok()?,
                    text.unwrap_or_default().trim().to_owned(),
                ))
            })
            .collect();
        lists.push((column, cells));
    }

    let result = Expected {
        orders: count_rows(c, "Catalog").await,
        deleted: count_text(&deleted, TRUE_VALUES),
        need_call: count_text(&image_key, TRUE_VALUES),
        need_parts: count_text(&wait, TRUE_VALUES),
        clients_map: count_rows(c, "ClientsMap").await,
        blacklisted: count_text(&blist, TRUE_VALUES),
        history: count_rows(c, "StatesMap").await + count_rows(c, "HistoryBD").await,
        next_order_id: sequence.max(ids.iter().copied().max().unwrap_or(0)) + 1,
        stock_rows: count_rows(c, "Stock").await,
        stock_map_rows: count_rows(c, "StockMap").await,
        money,
        lists,
    };
    conn.close().await.unwrap();
    result
}

async fn scalar(core: &Core, sql: &str) -> i64 {
    let db = core.db();
    db.query_one_raw(Statement::from_string(db.get_database_backend(), sql))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "n")
        .unwrap()
}

/// Импорт и сверка; расхождения собираются, а не роняют тест, чтобы схему PostgreSQL можно было убрать.
async fn check_backend(
    label: &str,
    core: &Core,
    source: &Path,
    want: &Expected,
) -> Result<(), Vec<String>> {
    let data = tempfile::tempdir().unwrap();
    let options = ImportOptions {
        source: source.to_path_buf(),
        attachments_dir: data.path().join("attachments"),
        backups_dir: data.path().join("backups"),
        replace: false,
        progress: None,
        cancel: None,
    };
    let started = Instant::now();
    let report = match import_folder(core, &Actor::unrestricted("real_db"), &options).await {
        Ok(r) => r,
        Err(_) => return Err(vec![format!("{label}: импорт завершился ошибкой")]),
    };
    println!(
        "[{label}] время импорта, с — {:.1}",
        started.elapsed().as_secs_f64()
    );

    let s = &report.summary;
    let need_call = scalar(core, "SELECT COUNT(*) AS n FROM orders WHERE need_call").await;
    let need_parts = scalar(core, "SELECT COUNT(*) AS n FROM orders WHERE need_parts").await;
    let blacklisted = scalar(core, "SELECT COUNT(*) AS n FROM clients WHERE blacklisted").await;
    let money = {
        let mut out = [0i64; 5];
        for (i, column) in [
            "estimate_from",
            "prepayment",
            "final_price",
            "extra_costs",
            "discount",
        ]
        .into_iter()
        .enumerate()
        {
            out[i] = scalar(
                core,
                &format!("SELECT CAST(COALESCE(SUM({column}), 0) AS BIGINT) AS n FROM orders"),
            )
            .await;
        }
        out
    };
    let without_client = report.count(WarningCode::OrderWithoutClient);
    // Запчасти, вычтенные из затрат: по заказам без «расходы оставлены как есть».
    let kept: HashSet<i64> = report
        .warnings
        .iter()
        .filter(|w| w.code == WarningCode::ExtraCostsKept)
        .filter_map(|w| w.key.parse().ok())
        .collect();
    let split: i64 = {
        let db = core.db();
        db.query_all_raw(Statement::from_string(
            db.get_database_backend(),
            "SELECT order_id, CAST(SUM(price * quantity) AS BIGINT) AS n FROM order_parts \
             GROUP BY order_id",
        ))
        .await
        .unwrap()
        .into_iter()
        .map(|row| {
            let order: i64 = row.try_get_by_index(0).unwrap();
            let sum: i64 = row.try_get_by_index(1).unwrap();
            (order, sum)
        })
        .filter(|(order, _)| !kept.contains(order))
        .map(|(_, sum)| sum)
        .sum()
    };

    println!("[{label}] заказов — {}", s.orders);
    println!("[{label}] удалённых — {}", s.deleted_orders);
    println!("[{label}] клиентов — {}", s.clients);
    println!(
        "[{label}] клиентов сверх ClientsMap — {}",
        s.clients.saturating_sub(want.clients_map)
    );
    println!("[{label}] без клиента — {without_client}");
    println!("[{label}] телефонов клиентов — {}", s.client_phones);
    println!("[{label}] записей истории — {}", s.history);
    println!("[{label}] статусов — {}", s.statuses);
    println!("[{label}] need_call — {need_call}");
    println!("[{label}] need_parts — {need_parts}");
    println!("[{label}] в чёрном списке — {blacklisted}");
    println!("[{label}] следующий номер — {}", s.next_order_id);
    println!("[{label}] строк legacy_rows — {}", s.legacy_rows);
    println!("[{label}] позиций склада — {}", s.stock_items);
    println!("[{label}] запчастей в заказах — {}", s.order_parts);
    println!("[{label}] вычтено из затрат запчастей, коп. — {split}");
    println!("[{label}] затраты оставлены как есть — {}", kept.len());
    println!("[{label}] деньги, коп. (оценка, предоплата, итог, затраты, скидка) — {money:?}");
    println!(
        "[{label}] файлов: скопировано {}, не найдено {}",
        report.files.copied, report.files.missing
    );
    for code in WarningCode::ALL {
        let n = report.count(*code);
        if n > 0 {
            println!("[{label}] предупреждение {} — {n}", code.as_str());
        }
    }
    // Справочники из заказов: название справочника и числа — подписи программы, не значения базы;
    // строка с другими словами не печатается.
    let mut unsafe_lines = 0;
    for w in report
        .warnings
        .iter()
        .filter(|w| w.code == WarningCode::DictionaryFromOrders)
    {
        if dictionary_line_is_safe(&w.key, &w.value) {
            println!("[{label}] справочник «{}» — {}", w.key, w.value);
        } else {
            unsafe_lines += 1;
        }
    }
    // Поля-списки, приведённые к виду «пункт, пункт»: сколько заказов изменили текст.
    for (column, cells) in &want.lists {
        let db = core.db();
        let rows = db
            .query_all_raw(Statement::from_string(
                db.get_database_backend(),
                format!("SELECT id, \"{column}\" FROM orders"),
            ))
            .await
            .unwrap();
        let (mut filled, mut changed) = (0, 0);
        for row in rows {
            let id: i64 = row.try_get_by_index(0).unwrap();
            let text: String = row.try_get_by_index(1).unwrap();
            let source = cells.get(&id).map(String::as_str).unwrap_or_default();
            filled += u64::from(!source.is_empty());
            changed += u64::from(text != source);
        }
        println!(
            "[{label}] поле-список {column}: непустых {filled}, приведено к «пункт, пункт» {changed}"
        );
    }

    let mut bad = Vec::new();
    if unsafe_lines > 0 {
        bad.push(format!(
            "{label}: строк справочников не только с числами и подписями — {unsafe_lines}"
        ));
    }
    let mut eq = |name: &str, got: i128, expect: i128| {
        if got != expect {
            bad.push(format!(
                "{label}: {name}: получено {got}, ожидалось {expect}"
            ));
        }
    };
    eq("заказов", s.orders.into(), want.orders.into());
    eq("удалённых", s.deleted_orders.into(), want.deleted.into());
    eq("need_call", need_call.into(), want.need_call.into());
    eq("need_parts", need_parts.into(), want.need_parts.into());
    eq(
        "в чёрном списке",
        blacklisted.into(),
        want.blacklisted.into(),
    );
    eq("записей истории", s.history.into(), want.history.into());
    eq(
        "следующий номер",
        s.next_order_id.into(),
        want.next_order_id.into(),
    );
    for (i, name) in [
        "estimate_from",
        "prepayment",
        "final_price",
        "extra_costs",
        "discount",
    ]
    .into_iter()
    .enumerate()
    {
        // Затраты MyWork 2 = расходы MyWork 3 + вычтенные из них запчасти.
        let got = if name == "extra_costs" {
            money[i] + split
        } else {
            money[i]
        };
        eq(name, got.into(), want.money[i].into());
    }
    let skipped = report.count(WarningCode::StockRowSkipped) as u64;
    let without_order = report.count(WarningCode::StockPartWithoutOrder) as u64;
    eq(
        "позиций склада и пропущенных строк",
        (s.stock_items + skipped).into(),
        want.stock_rows.into(),
    );
    eq(
        "запчастей и списаний без заказа",
        (s.order_parts + without_order).into(),
        want.stock_map_rows.into(),
    );
    if s.clients < want.clients_map {
        bad.push(format!(
            "{label}: клиентов {} меньше строк ClientsMap {}",
            s.clients, want.clients_map
        ));
    }
    if bad.is_empty() { Ok(()) } else { Err(bad) }
}

#[test]
fn a_dictionary_line_with_other_words_is_not_printed() {
    assert!(dictionary_line_is_safe(
        "Неисправности",
        "из заказов — 2, из файла — 1, пунктов из одного заказа — 7, заказов с переписанным полем — 4"
    ));
    assert!(dictionary_line_is_safe(
        "Откуда узнали",
        "из клиентов — 1, стандартных — 4, значений у одного клиента — 7"
    ));
    assert!(dictionary_line_is_safe(
        "Типы запчастей",
        "из файла — 12, со склада — 3"
    ));
    assert!(!dictionary_line_is_safe(
        "Бренды",
        "из заказов — 3, Nokia — 1"
    ));
    assert!(!dictionary_line_is_safe("Иванов", "из заказов — 3"));
}

#[tokio::test]
#[ignore = "нужна настоящая папка MyWork 2: MW_LEGACY_REAL_DIR"]
async fn real_mywork2_folder_imports_without_loss() {
    let Some(dir) = std::env::var_os("MW_LEGACY_REAL_DIR") else {
        return;
    };
    let source = PathBuf::from(dir);
    let want = expected(&find_db(&source)).await;
    println!(
        "[источник] заказов {}, удалённых {}, ClientsMap {}, история {}, следующий номер {}",
        want.orders, want.deleted, want.clients_map, want.history, want.next_order_id
    );

    let want = Arc::new(want);
    let mut failures = Vec::new();
    for (label, db) in mw_db::testing::all_backends().await {
        let is_postgres = db.get_database_backend() == DatabaseBackend::Postgres;
        // Имя схемы узнаём до импорта: уборка не должна зависеть от того, что произойдёт дальше.
        let schema = if is_postgres {
            query_text(&db, "SELECT current_schema() AS n").await
        } else {
            None
        };
        if is_postgres && schema.is_none() {
            // Настоящие данные нельзя писать в схему, которую потом не удастся удалить.
            failures.push(format!("{label}: имя схемы не определено, импорт пропущен"));
            continue;
        }
        // Паника внутри задачи становится `JoinError`; её текст может содержать данные и не печатается.
        let task = tokio::spawn({
            let (db, source, want) = (db.clone(), source.clone(), Arc::clone(&want));
            async move {
                let clock = Arc::new(FixedClock::new(
                    Utc.with_ymd_and_hms(2026, 11, 11, 9, 0, 0).unwrap(),
                ));
                let core = Core::open(db, clock, "UTC").await.unwrap();
                check_backend(label, &core, &source, &want).await
            }
        });
        match task.await {
            Ok(Ok(())) => {}
            Ok(Err(bad)) => failures.extend(bad),
            Err(_) => failures.push(format!("{label}: проверка прервалась паникой")),
        }
        if is_postgres {
            // Настоящие данные клиентов не должны остаться в тестовой базе.
            match &schema {
                Some(schema) => match db
                    .execute_unprepared(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
                    .await
                {
                    Ok(_) => println!("[{label}] схема удалена"),
                    Err(_) => failures.push(format!("{label}: схему не удалось удалить")),
                },
                None => failures.push(format!("{label}: имя схемы не определено")),
            }
        }
    }
    assert!(failures.is_empty(), "расхождения:\n{}", failures.join("\n"));
}

async fn query_text(db: &DatabaseConnection, sql: &str) -> Option<String> {
    db.query_one_raw(Statement::from_string(db.get_database_backend(), sql))
        .await
        .ok()??
        .try_get("", "n")
        .ok()
}
