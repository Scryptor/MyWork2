//! Помощники модульных тестов.

use std::collections::BTreeMap;
use std::path::Path;

use sha2::{Digest, Sha256};
use tempfile::TempDir;

use crate::fixture::{FixtureOptions, FixtureVersion, write_fixture};
use crate::folder::{LegacyFolder, scan_folder};
use crate::report::Warnings;
use crate::source::{LegacyDb, read_database};

/// Собирает синтетическую папку MyWork 2 указанной версии во временной папке.
pub(crate) async fn fixture(version: FixtureVersion) -> TempDir {
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
    dir
}

/// Путь (через `/`) → sha256 содержимого для каждого файла дерева.
pub(crate) fn hash_tree(path: &Path) -> BTreeMap<String, String> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let relative = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(
                    relative,
                    hex::encode(Sha256::digest(std::fs::read(&path).unwrap())),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(path, path, &mut out);
    out
}

/// Фикстура указанной версии, прочитанная так же, как читает импорт: база и папка.
pub(crate) async fn prepared(version: FixtureVersion) -> (LegacyDb, LegacyFolder) {
    let dir = fixture(version).await;
    let folder = scan_folder(dir.path()).unwrap();
    let db = read_database(&folder.db_file).await.unwrap();
    // Папка фикстуры удаляется вместе с `dir`; тесты используют только уже прочитанное.
    (db, folder)
}

/// Пары (код, ключ) предупреждений по порядку.
pub(crate) fn codes(w: &Warnings) -> Vec<(&str, &str)> {
    w.list()
        .iter()
        .map(|x| (x.code.as_str(), x.key.as_str()))
        .collect()
}

/// Преобразование фикстуры указанной версии с временем импорта `2026-11-12 10:00:00`.
pub(crate) async fn conversion_of(version: FixtureVersion) -> crate::convert::Conversion {
    let dir = fixture(version).await;
    let folder = scan_folder(dir.path()).unwrap();
    let db = read_database(&folder.db_file).await.unwrap();
    crate::convert::convert(&db, &folder, dt("2026-11-12 10:00:00"))
}

pub(crate) fn dt(text: &str) -> chrono::NaiveDateTime {
    chrono::NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M:%S").unwrap()
}

pub(crate) fn rub(rubles: i64) -> mw_domain::money::Money {
    mw_domain::money::Money::from_rubles(rubles)
}

pub(crate) fn order(b: &mw_core::ImportBundle, id: i64) -> &mw_core::ImportedOrder {
    b.orders.iter().find(|o| o.id == id).unwrap()
}

pub(crate) fn client(b: &mw_core::ImportBundle, id: i64) -> &mw_core::ImportedClient {
    b.clients.iter().find(|c| c.id == id).unwrap()
}

/// Как `conversion_of`, но перед чтением выполняет SQL над базой фикстуры.
pub(crate) async fn conversion_with_sql(
    version: FixtureVersion,
    statements: &[&str],
) -> crate::convert::Conversion {
    use sqlx::Connection;
    let dir = fixture(version).await;
    let db_path = dir.path().join("baza.sqlite");
    let mut conn = sqlx::SqliteConnection::connect(&format!("sqlite://{}", db_path.display()))
        .await
        .unwrap();
    for sql in statements {
        sqlx::query(sqlx::AssertSqlSafe((*sql).to_owned()))
            .execute(&mut conn)
            .await
            .unwrap();
    }
    conn.close().await.unwrap();
    let folder = scan_folder(dir.path()).unwrap();
    let db = read_database(&folder.db_file).await.unwrap();
    crate::convert::convert(&db, &folder, dt("2026-11-12 10:00:00"))
}

/// Как `conversion_of`, но перед чтением правит текстовый файл фикстуры (Windows-1251) функцией `edit`.
pub(crate) async fn conversion_with_text(
    version: FixtureVersion,
    relative: &str,
    edit: &dyn Fn(&str) -> String,
) -> crate::convert::Conversion {
    let dir = fixture(version).await;
    let path = dir.path().join(relative);
    let (text, _) = crate::text::decode_text(&std::fs::read(&path).unwrap());
    let edited = edit(&text);
    assert_ne!(edited, text, "правка не изменила {relative}");
    std::fs::write(&path, encoding_rs::WINDOWS_1251.encode(&edited).0).unwrap();
    let folder = scan_folder(dir.path()).unwrap();
    let db = read_database(&folder.db_file).await.unwrap();
    crate::convert::convert(&db, &folder, dt("2026-11-12 10:00:00"))
}
