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
