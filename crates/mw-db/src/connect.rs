//! Подключение к SQLite или PostgreSQL по URL.

use std::path::Path;
use std::time::Duration;

use sea_orm::sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous};
use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr};

/// URL SQLite-файла: файл создаётся при отсутствии. Каждый байт пути, кроме `A-Za-z0-9-._~` и `/`,
/// кодируется как `%XX` (sqlx декодирует имя файла): так URL проходит `url::Url::parse`
/// для путей Windows (диск, UNC, `\\?\`, пробелы, кириллица) и Unix.
pub fn sqlite_url(path: &Path) -> String {
    let mut url = String::from("sqlite://");
    for byte in path.to_string_lossy().bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/') {
            url.push(char::from(byte));
        } else {
            url.push_str(&format!("%{byte:02X}"));
        }
    }
    url.push_str("?mode=rwc");
    url
}

/// Pragma каждого SQLite-соединения (`journal_mode` задаётся отдельно: у базы в памяти нет WAL).
pub(crate) fn sqlite_pragmas(
    options: SqliteConnectOptions,
    journal_mode: SqliteJournalMode,
) -> SqliteConnectOptions {
    options
        .foreign_keys(true)
        .journal_mode(journal_mode)
        .busy_timeout(Duration::from_millis(5000))
        .synchronous(SqliteSynchronous::Normal)
}

/// Открывает базу. `sqlite:` — с pragma на каждом соединении, `postgres:` — обычное подключение.
pub async fn connect(url: &str) -> Result<DatabaseConnection, DbErr> {
    let mut options = ConnectOptions::new(url);
    if url.starts_with("sqlite:") {
        options.max_connections(5);
        options.map_sqlx_sqlite_opts(|o| {
            sqlite_pragmas(o.create_if_missing(true), SqliteJournalMode::Wal)
        });
    }
    Database::connect(options).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("mw-db-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn sqlite_url_is_accepted_by_url_and_sqlx_for_unix_and_windows_paths() {
        let cases = [
            "/var/lib/my work/a#b%c.db",
            r"C:\Program Files\Мастер\mw.db",
            r"\\server\share\mw.db",
            r"\\?\C:\data\mw.db",
            r"data\mw.db",
        ];
        for case in cases {
            let u = sqlite_url(Path::new(case));
            assert!(url::Url::parse(&u).is_ok(), "Url::parse отклонил {u}");
            let options = u.parse::<SqliteConnectOptions>();
            assert!(options.is_ok(), "sqlx отклонил {u}");
            let name = options
                .unwrap()
                .get_filename()
                .to_string_lossy()
                .into_owned();
            assert_eq!(name, case, "имя файла не восстановилось из {u}");
        }
    }

    #[tokio::test]
    async fn connect_creates_file_for_url_with_space_and_cyrillic() {
        let path = temp_dir("url").join("папка с пробелом").join("t.db");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        connect(&sqlite_url(&path)).await.unwrap();
        assert!(path.exists());
    }

    #[tokio::test]
    async fn connect_itself_creates_missing_file_without_mode_rwc() {
        let path = temp_dir("create").join("new.db");
        let url = sqlite_url(&path);
        let bare = url.split('?').next().unwrap();
        assert!(!bare.contains("mode"));
        connect(bare).await.unwrap();
        assert!(path.exists());
    }
}
