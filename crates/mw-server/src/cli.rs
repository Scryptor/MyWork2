use crate::admin::{create_admin, read_password};
use crate::config::{Config, DEFAULT_LISTEN, ServerError, redact_database_url};
use crate::paths::{os_data_dir, resolve_data_dir};
use crate::server::{Server, connect_and_migrate, shutdown_signal, system_timezone};
use clap::{Parser, Subcommand};
use mw_core::{Core, SystemClock};
use std::io::IsTerminal;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

/// Сервер MyWork.
#[derive(Parser)]
#[command(name = "mw-server", version, about = "Сервер MyWork")]
pub struct Cli {
    /// Адрес и порт, на которых слушает сервер.
    #[arg(long, env = "MW_LISTEN", default_value = DEFAULT_LISTEN)]
    pub listen: SocketAddr,
    /// Папка данных (по умолчанию — папка данных ОС или `data` рядом с программой при `portable.txt`).
    #[arg(long, env = "MW_DATA_DIR")]
    pub data_dir: Option<PathBuf>,
    /// Адрес базы данных (по умолчанию — SQLite-файл в папке данных).
    #[arg(long, env = "MW_DATABASE_URL", hide_env_values = true)]
    pub database_url: Option<String>,
    /// Часовой пояс предприятия при первом запуске (по умолчанию — пояс системы).
    #[arg(long, env = "MW_TIMEZONE")]
    pub timezone: Option<String>,
    #[command(subcommand)]
    pub command: Option<Command>,
}

impl std::fmt::Debug for Cli {
    // Адрес базы может содержать пароль, поэтому в отладочный вывод он попадает только скрытым.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cli")
            .field("listen", &self.listen)
            .field("data_dir", &self.data_dir)
            .field(
                "database_url",
                &self.database_url.as_deref().map(redact_database_url),
            )
            .field("timezone", &self.timezone)
            .field("command", &self.command)
            .finish()
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Запустить сервер (по умолчанию).
    Serve,
    /// Создать администратора; пароль запрашивается в терминале или читается из стандартного ввода.
    CreateAdmin {
        #[arg(long)]
        login: String,
        #[arg(long, default_value = "")]
        name: String,
    },
}

impl Cli {
    pub fn config(&self) -> Result<Config, ServerError> {
        let exe = std::env::current_exe().ok();
        let exe_dir = exe.as_deref().and_then(|path| path.parent());
        Ok(Config {
            listen: self.listen,
            data_dir: resolve_data_dir(self.data_dir.clone(), exe_dir, os_data_dir())?,
            database_url: self.database_url.clone(),
            timezone: self.timezone.clone(),
        })
    }
}

pub async fn run(cli: Cli) -> Result<(), ServerError> {
    let config = cli.config()?;
    match cli.command {
        None | Some(Command::Serve) => Server::start(&config).await?.serve(shutdown_signal()).await,
        Some(Command::CreateAdmin { login, name }) => {
            let password = ask_password()?;
            let db = connect_and_migrate(&config).await?;
            let core =
                Core::open(db.clone(), Arc::new(SystemClock), &system_timezone(&config)).await?;
            let result = create_admin(&core, &login, &name, &password).await;
            drop(core);
            let _ = db.close().await;
            let user = result?;
            println!("Администратор создан: {}", user.login);
            Ok(())
        }
    }
}

fn ask_password() -> Result<String, ServerError> {
    let io_error = |source| ServerError::Io {
        context: "не удалось прочитать пароль".into(),
        source,
    };
    if std::io::stdin().is_terminal() {
        let first = rpassword::prompt_password("Пароль: ").map_err(io_error)?;
        let second = rpassword::prompt_password("Повторите пароль: ").map_err(io_error)?;
        if first != second {
            return Err(ServerError::Config("пароли не совпадают".into()));
        }
        if first.is_empty() {
            return Err(ServerError::Config("пароль не может быть пустым".into()));
        }
        Ok(first)
    } else {
        read_password(&mut std::io::stdin().lock())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_output_hides_the_database_password() {
        let cli =
            Cli::try_parse_from(["mw-server", "--database-url", "postgres://mw:secret@h/db"])
                .unwrap();
        assert!(!format!("{cli:?}").contains("secret"));
    }

    #[test]
    fn parses_defaults_and_create_admin() {
        let cli = Cli::try_parse_from(["mw-server"]).unwrap();
        assert!(cli.command.is_none());
        assert_eq!(cli.listen, "127.0.0.1:8080".parse::<SocketAddr>().unwrap());
        let cli = Cli::try_parse_from([
            "mw-server",
            "--listen",
            "0.0.0.0:9000",
            "create-admin",
            "--login",
            "boss",
        ])
        .unwrap();
        assert!(
            matches!(cli.command, Some(Command::CreateAdmin { ref login, .. }) if login == "boss")
        );
    }
}
