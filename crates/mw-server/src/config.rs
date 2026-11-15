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

/// Скрывает секреты в адресе базы: пароль и параметры с `password` в имени заменяются на `***`;
/// неразбираемый адрес сводится к `схема://***`. Адрес без секретов возвращается как есть.
pub fn redact_database_url(url: &str) -> String {
    let known = |parsed: &url::Url| {
        matches!(parsed.scheme(), "postgres" | "postgresql" | "sqlite") && {
            parsed.has_host() || parsed.scheme() == "sqlite"
        }
    };
    let mut parsed = match url::Url::parse(url) {
        Ok(parsed) if known(&parsed) => parsed,
        _ => {
            return match url.split_once("://") {
                Some((scheme, _)) => format!("{scheme}://***"),
                None => "***".to_string(),
            };
        }
    };
    let mut changed = false;
    if parsed.password().is_some() && parsed.set_password(Some("***")).is_ok() {
        changed = true;
    }
    let has_secret_param = parsed
        .query_pairs()
        .any(|(name, _)| name.to_lowercase().contains("password"));
    if has_secret_param {
        let pairs: Vec<(String, String)> = parsed
            .query_pairs()
            .map(|(name, value)| {
                let value = if name.to_lowercase().contains("password") {
                    "***".to_string()
                } else {
                    value.into_owned()
                };
                (name.into_owned(), value)
            })
            .collect();
        parsed.query_pairs_mut().clear().extend_pairs(pairs);
        changed = true;
    }
    if changed {
        parsed.to_string()
    } else {
        url.to_string()
    }
}

/// Заменяет адрес базы в тексте ошибки его скрытой формой.
pub fn scrub_url(message: &str, url: &str) -> String {
    let redacted = redact_database_url(url);
    if url.is_empty() || redacted == url {
        return message.to_string();
    }
    message.replace(url, &redacted)
}

/// Проверяет имя часового пояса (IANA); общая для командной строки и сервера.
pub fn validate_timezone(name: &str) -> Result<(), ServerError> {
    if name.parse::<chrono_tz::Tz>().is_err() {
        return Err(ServerError::Config(format!(
            "неизвестный часовой пояс «{name}»: укажите имя IANA, например Europe/Moscow"
        )));
    }
    Ok(())
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
    #[error("{}", crate::import::import_error_message(.0))]
    Import(mw_legacy::LegacyError),
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
        assert_eq!(redact_database_url("not a url"), "***");
        assert_eq!(
            redact_database_url("sqlite:///data/mywork.db?mode=rwc"),
            "sqlite:///data/mywork.db?mode=rwc"
        );
    }

    #[test]
    fn redaction_hides_secrets_in_odd_urls() {
        let out = redact_database_url("postgres://mw:Pa55#word@db.invalid:5432/mw");
        assert_eq!(out, "postgres://***");
        let out = redact_database_url("postgres://mw:a/b?c@h/db");
        assert!(
            !out.contains("a/b") && out.starts_with("postgres://"),
            "{out}"
        );
        let out = redact_database_url("postgres://db/mw?password=S3cr3t&user=mw");
        assert!(!out.contains("S3cr3t"), "{out}");
        assert!(
            out.contains("user=mw") && out.contains("password=***"),
            "{out}"
        );
        let out = redact_database_url("postgres://mw:pw@db/mw?sslPassword=zzz");
        assert!(!out.contains("zzz") && !out.contains("pw@"), "{out}");
    }

    #[test]
    fn redaction_hides_schemeless_credentials() {
        let out = redact_database_url("mw:secret@localhost:5432/mw");
        assert!(!out.contains("secret"), "{out}");
        assert_eq!(redact_database_url("mw://u:secret@h/db"), "mw://***");
    }

    #[test]
    fn scrub_of_empty_url_changes_nothing() {
        assert_eq!(scrub_url("ошибка", ""), "ошибка");
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
