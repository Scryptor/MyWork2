use mw_core::CoreError;
use std::net::SocketAddr;
use std::path::PathBuf;

pub const DATA_DIR_NAME: &str = "MyWork3";
pub const DB_FILE_NAME: &str = "mywork.db";
pub const DEFAULT_LISTEN: &str = "127.0.0.1:8080";

/// Настройки запуска сервера.
#[derive(Clone)]
pub struct Config {
    pub listen: SocketAddr,
    pub data_dir: PathBuf,
    pub database_url: Option<String>,
    pub timezone: Option<String>,
}

impl std::fmt::Debug for Config {
    // Адрес базы может содержать пароль, поэтому в отладочный вывод он попадает только скрытым.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("listen", &self.listen)
            .field("data_dir", &self.data_dir)
            .field(
                "database_url",
                &self.database_url.as_deref().map(redact_database_url),
            )
            .field("timezone", &self.timezone)
            .finish()
    }
}

impl Config {
    /// Явный адрес базы или SQLite-файл в папке данных.
    pub fn database_url(&self) -> String {
        match &self.database_url {
            Some(url) => url.clone(),
            None => mw_db::sqlite_url(&self.data_dir.join(DB_FILE_NAME)),
        }
    }
}

/// Заменяет пароль в адресе базы на `***`; адрес без пароля или неразбираемый возвращается как есть.
pub fn redact_database_url(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(mut parsed) if parsed.password().is_some() => {
            if parsed.set_password(Some("***")).is_ok() {
                parsed.to_string()
            } else {
                url.to_string()
            }
        }
        _ => url.to_string(),
    }
}

/// Ошибки запуска и служебных команд сервера.
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("{0}")]
    Config(String),
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{0}")]
    Database(String),
    #[error(transparent)]
    Core(#[from] CoreError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn database_url_defaults_and_redaction() {
        assert_eq!(
            Config {
                listen: DEFAULT_LISTEN.parse().unwrap(),
                data_dir: "/d".into(),
                database_url: None,
                timezone: None
            }
            .database_url(),
            mw_db::sqlite_url(Path::new("/d/mywork.db"))
        );
        assert_eq!(
            redact_database_url("postgres://mw:s3cr3t@db:5432/mw"),
            "postgres://mw:***@db:5432/mw"
        );
        assert_eq!(redact_database_url("postgres://db/mw"), "postgres://db/mw");
        assert_eq!(
            redact_database_url("sqlite:///data/mywork.db?mode=rwc"),
            "sqlite:///data/mywork.db?mode=rwc"
        );
    }

    #[test]
    fn debug_output_hides_the_password() {
        let config = Config {
            listen: DEFAULT_LISTEN.parse().unwrap(),
            data_dir: "/d".into(),
            database_url: Some("postgres://mw:s3cr3t@db/mw".into()),
            timezone: None,
        };
        assert!(!format!("{config:?}").contains("s3cr3t"));
    }
}
