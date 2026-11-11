//! Подключение к SQLite или PostgreSQL по URL.

use std::path::Path;
use std::time::Duration;

use sea_orm::sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous};
use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr};

/// URL SQLite-файла: файл создаётся при отсутствии. Путь кодируется так, как его разбирает sqlx
/// (после `sqlite://` идёт имя файла «как есть», в нём декодируются только `%XX`), поэтому
/// обратные слэши и буква диска Windows проходят без изменений.
pub fn sqlite_url(path: &Path) -> String {
    let mut url = String::from("sqlite://");
    for ch in path.to_string_lossy().chars() {
        match ch {
            '%' => url.push_str("%25"),
            '?' => url.push_str("%3F"),
            '#' => url.push_str("%23"),
            _ => url.push(ch),
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
        options.map_sqlx_sqlite_opts(|o| sqlite_pragmas(o, SqliteJournalMode::Wal));
    }
    Database::connect(options).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filename(url: &str) -> Result<String, sea_orm::sqlx::Error> {
        let options: SqliteConnectOptions = url.parse()?;
        Ok(options.get_filename().to_string_lossy().into_owned())
    }

    #[test]
    fn sqlite_url_is_accepted_by_sqlx_for_unix_and_windows_paths() {
        let unix = sqlite_url(Path::new("/var/lib/my work/a#b.db"));
        assert_eq!(unix, "sqlite:///var/lib/my work/a%23b.db?mode=rwc");
        assert_eq!(filename(&unix).unwrap(), "/var/lib/my work/a#b.db");

        let windows = sqlite_url(Path::new(r"C:\Users\Мастер\mw.db"));
        assert_eq!(windows, r"sqlite://C:\Users\Мастер\mw.db?mode=rwc");
        assert_eq!(filename(&windows).unwrap(), r"C:\Users\Мастер\mw.db");
    }
}
