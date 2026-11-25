use crate::admin::{create_admin, read_password};
use crate::config::{Config, DEFAULT_LISTEN, ServerError, redact_database_url, validate_timezone};
use crate::import::run_import_with_db;
use crate::lock::{DataDirLock, LockError};
use crate::paths::{os_data_dir, resolve_data_dir};
use crate::server::{
    Server, connect_and_migrate, core_message, log_timezone, shutdown_signal, system_timezone,
};
use clap::{Parser, Subcommand};
use mw_core::{Actor, BackupKind, Core, SystemClock};
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
    /// Папка данных (по умолчанию — `MyWork3` в папке данных ОС, у отладочной сборки — `MyWork3-dev`;
    /// `data` рядом с программой при `portable.txt`).
    #[arg(long, env = "MW_DATA_DIR")]
    pub data_dir: Option<PathBuf>,
    /// Адрес базы данных (по умолчанию — SQLite-файл в папке данных).
    #[arg(long, env = "MW_DATABASE_URL", hide_env_values = true)]
    pub database_url: Option<String>,
    /// Часовой пояс предприятия при первом запуске (по умолчанию — пояс системы).
    #[arg(long, env = "MW_TIMEZONE")]
    pub timezone: Option<String>,
    /// Папка собранного интерфейса: сервер отдаёт его с того же адреса, что и API.
    #[arg(long, env = "MW_UI_DIR")]
    pub ui_dir: Option<PathBuf>,
    /// Папка журнала (по умолчанию — `logs` в папке данных); `none` — писать только в вывод.
    #[arg(long, global = true, value_name = "ПАПКА|none")]
    pub log_dir: Option<String>,
    /// Завершаться, когда закрыт стандартный ввод (родитель умер); для тестов и десктопа.
    /// Если ввод уже закрыт или это /dev/null, сервер завершится сразу после запуска.
    #[arg(long, global = true, hide = true)]
    pub exit_on_stdin_close: bool,
    /// Только для тестов: при запуске отметить программу настроенной, если отметки ещё нет.
    #[arg(long, global = true, hide = true)]
    pub assume_setup: bool,
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
            .field("ui_dir", &self.ui_dir)
            .field("log_dir", &self.log_dir)
            .field("exit_on_stdin_close", &self.exit_on_stdin_close)
            .field("assume_setup", &self.assume_setup)
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
    /// Перенести данные из папки MyWork 2 (или файла baza.sqlite в ней). Импорт — только в пустую базу; --replace заменяет данные SQLite-базы, сначала сохранив копию.
    ImportLegacy {
        /// Папка MyWork 2 или файл baza.sqlite в ней.
        source: PathBuf,
        #[arg(long)]
        replace: bool,
    },
    /// Сохранить резервную копию данных; программа при этом может работать.
    Backup,
    /// Восстановить данные из копии (папка с файлом mywork-backup.json). Прежние данные сохраняются
    /// в папке копий. Программу перед этим нужно закрыть.
    Restore {
        /// Папка копии.
        copy: PathBuf,
    },
}

impl Cli {
    /// Запускается ли сервер (подкоманда `serve` или без подкоманды).
    pub fn is_serve(&self) -> bool {
        matches!(self.command, None | Some(Command::Serve))
    }

    /// Папка журнала: `--log-dir`, по умолчанию `logs` в папке данных; `none` — без файла.
    pub fn resolve_log_dir(&self, data_dir: &std::path::Path) -> Option<PathBuf> {
        match self.log_dir.as_deref() {
            Some(value) if value.eq_ignore_ascii_case("none") => None,
            Some(value) => Some(PathBuf::from(value)),
            None => Some(data_dir.join("logs")),
        }
    }

    pub fn config(&self) -> Result<Config, ServerError> {
        if let Some(name) = &self.timezone {
            validate_timezone(name)?;
        }
        if self
            .database_url
            .as_deref()
            .is_some_and(|u| u.trim().is_empty())
        {
            return Err(ServerError::Config(
                "адрес базы данных пуст: укажите его или не задавайте --database-url / MW_DATABASE_URL".into(),
            ));
        }
        let exe = std::env::current_exe().ok();
        let exe_dir = exe.as_deref().and_then(|path| path.parent());
        Ok(Config {
            listen: self.listen,
            data_dir: resolve_data_dir(
                self.data_dir.clone(),
                exe_dir,
                os_data_dir(),
                cfg!(debug_assertions),
            )?,
            database_url: self.database_url.clone(),
            timezone: self.timezone.clone(),
            ui_dir: self.ui_dir.clone(),
            // Процесс-рендерер — эта же программа с аргументом `RENDER_WORKER_ARG`.
            render_worker: exe,
        })
    }
}

/// Ждёт закрытия стандартного ввода: так сервер узнаёт, что родительский процесс умер.
/// Читает обычный поток, а не задачу рантайма: зависшее чтение не задерживает выход процесса.
async fn stdin_closed() {
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        use std::io::Read;
        let mut sink = [0u8; 256];
        let mut stdin = std::io::stdin().lock();
        while matches!(stdin.read(&mut sink), Ok(n) if n > 0) {}
        let _ = tx.send(());
    });
    if rx.await.is_err() {
        std::future::pending::<()>().await;
    }
    tracing::info!("стандартный ввод закрыт: родительский процесс завершился");
}

/// Замок папки данных на всё время команды: второй сервер или импорт на той же папке отказывают сразу,
/// до базы (`lock`).
fn lock_data_dir(config: &Config) -> Result<DataDirLock, ServerError> {
    crate::lock::acquire(&config.data_dir).map_err(|error| match error {
        LockError::Busy { .. } => ServerError::Config(format!(
            "{error}: закройте её (программу MyWork или сервер mw-server) и повторите"
        )),
        LockError::Io { .. } => ServerError::Config(error.to_string()),
    })
}

pub async fn run(cli: Cli) -> Result<(), ServerError> {
    let config = cli.config()?;
    match cli.command {
        None | Some(Command::Serve) => {
            let _lock = lock_data_dir(&config)?;
            let server = Server::start(&config).await?;
            if cli.assume_setup {
                server.core().assume_setup_done().await?;
            }
            if cli.exit_on_stdin_close {
                let stop = async {
                    tokio::select! {
                        () = shutdown_signal() => {},
                        () = stdin_closed() => {},
                    }
                };
                server.serve(stop).await
            } else {
                server.serve(shutdown_signal()).await
            }
        }
        Some(Command::CreateAdmin { login, name }) => {
            let password = ask_password()?;
            let db = connect_and_migrate(&config).await?;
            let core =
                Core::open(db.clone(), Arc::new(SystemClock), &system_timezone(&config)).await?;
            log_timezone(&core, config.timezone.as_deref());
            let result = create_admin(&core, &login, &name, &password).await;
            drop(core);
            let _ = db.close().await;
            let user = result?;
            println!("Администратор создан: {}", user.login);
            Ok(())
        }
        Some(Command::ImportLegacy { source, replace }) => {
            // Сначала проверка старой папки: замок создаёт папку данных, а в старую папку ничего не пишется.
            crate::import::refuse_inside_source(&config, &source)?;
            let _lock = lock_data_dir(&config)?;
            let db = connect_and_migrate(&config).await?;
            let result = run_import_with_db(&config, db.clone(), &source, replace).await;
            let _ = db.close().await;
            let outcome = match result {
                Ok(outcome) => outcome,
                Err(
                    ServerError::ImportReportNotWritten { ref report, .. }
                    | ServerError::ImportFilesNotPlaced { ref report, .. },
                ) => {
                    // База уже импортирована: покажем сводку, чтобы было видно, что данные внутри.
                    println!("{}", report.summary_text());
                    return result.map(|_| ());
                }
                Err(error) => return Err(error),
            };
            println!("{}", outcome.report.summary_text());
            println!(
                "Полный отчёт (содержит данные клиентов): {}",
                outcome.text_report.display()
            );
            println!(
                "Отчёт в формате JSON (содержит данные клиентов): {}",
                outcome.json_report.display()
            );
            if outcome.open_mode {
                println!(
                    "Пользователей нет: на этом компьютере вход без пароля, как в MyWork 2. Для работы по сети создайте администратора: mw-server create-admin"
                );
            }
            if let Some(hint) = restart_hint(&config) {
                println!("{hint}");
            }
            Ok(())
        }
        Some(Command::Backup) => run_backup(&config).await,
        Some(Command::Restore { copy }) => run_restore(&config, copy).await,
    }
}

/// Кто записан в журнале и в копиях, сделанных командой.
const CLI_WHO: &str = "Командная строка";

/// Только для базы в папке данных; для остальных копии не делаются и не восстанавливаются.
fn require_data_dir_base(config: &Config) -> Result<(), ServerError> {
    if config.database_url.is_some() {
        return Err(ServerError::Config(
            "Резервные копии делаются только для базы в папке данных".into(),
        ));
    }
    Ok(())
}

async fn open_core(config: &Config) -> Result<(Core, sea_orm::DatabaseConnection), ServerError> {
    let db = connect_and_migrate(config).await?;
    match Core::open(db.clone(), Arc::new(SystemClock), &system_timezone(config)).await {
        Ok(core) => Ok((
            core.with_attachments_dir(config.data_dir.clone())
                .with_backups(),
            db,
        )),
        Err(e) => {
            let _ = db.close().await;
            Err(e.into())
        }
    }
}

/// `mw-server backup`: без замка папки данных — копия базы (`VACUUM INTO`) безопасна рядом с работающей
/// программой.
async fn run_backup(config: &Config) -> Result<(), ServerError> {
    require_data_dir_base(config)?;
    let (core, db) = open_core(config).await?;
    let actor = Actor::unrestricted(CLI_WHO);
    let result = async {
        let copy = core.create_backup(&actor, BackupKind::Manual).await?;
        let overview = core.backup_overview(&actor).await?;
        Ok::<_, mw_core::CoreError>(PathBuf::from(overview.folder).join(copy.name))
    }
    .await;
    drop(core);
    let _ = db.close().await;
    let path = result.map_err(|e| ServerError::Config(core_message(&e)))?;
    println!("Копия сохранена: {}", path.display());
    Ok(())
}

/// `mw-server restore <папка>`: под замком папки данных. Запрос и выполнение — сразу, без перезапуска.
/// Оборванное прежнее восстановление сначала доводится до конца: новый запрос стёр бы его подготовку, и
/// прежние данные разошлись бы по двум страховочным копиям. Не доводится — ошибка, новое не начинается.
async fn run_restore(config: &Config, copy: PathBuf) -> Result<(), ServerError> {
    require_data_dir_base(config)?;
    let _lock = lock_data_dir(config)?;
    let previous = mw_core::backups::apply_pending_restore(&config.data_dir)
        .await
        .map_err(|e| {
            ServerError::Config(format!(
                "Прежнее восстановление не закончено, новое не начато: {}",
                core_message(&e)
            ))
        })?;
    if let Some(previous) = previous {
        if previous.ok {
            record_in_audit(config, &previous).await;
            println!(
                "Прежнее восстановление закончено. {}",
                restored_line(config, &previous)
            );
        } else {
            println!(
                "Прежнее восстановление не выполнено, данные прежние: {}",
                previous.error.as_deref().unwrap_or("причина не указана")
            );
        }
    }
    let requested_at = request_time(config).await;
    let copy = std::fs::canonicalize(&copy).unwrap_or(copy);
    mw_core::backups::schedule_restore(&config.data_dir, &copy, requested_at, CLI_WHO)
        .await
        .map_err(|e| ServerError::Config(core_message(&e)))?;
    let result = mw_core::backups::apply_pending_restore(&config.data_dir)
        .await
        .map_err(|e| ServerError::Config(core_message(&e)))?
        .ok_or_else(|| ServerError::Config("восстановление не началось".into()))?;
    if !result.ok {
        return Err(ServerError::Config(
            result
                .error
                .unwrap_or_else(|| "копия не подошла, данные прежние".into()),
        ));
    }
    record_in_audit(config, &result).await;
    println!("{}", restored_line(config, &result));
    Ok(())
}

/// Запись удавшегося восстановления в журнал действий: сервер при запуске её уже не сделает, отметка снята.
async fn record_in_audit(config: &Config, result: &mw_core::BackupRestoreResult) {
    match open_core(config).await {
        Ok((core, db)) => {
            if let Err(e) = core.record_restore(result).await {
                tracing::warn!("восстановление не записано в журнал действий: {e}");
            }
            drop(core);
            let _ = db.close().await;
        }
        Err(e) => tracing::warn!("восстановление не записано в журнал действий: {e}"),
    }
}

/// «Данные восстановлены из копии от …. Прежние данные: <папка>».
fn restored_line(config: &Config, result: &mw_core::BackupRestoreResult) -> String {
    let mut line = format!(
        "Данные восстановлены из копии от {}.",
        result.copy.created_at.format("%d.%m.%Y %H:%M")
    );
    if let Some(safety) = &result.safety {
        let path = config
            .data_dir
            .join(mw_core::backups::DEFAULT_DIR)
            .join(safety);
        line.push_str(&format!(" Прежние данные: {}", path.display()));
    }
    line
}

/// Время запроса: часы текущей базы (местное время предприятия). Команда нужна и тогда, когда база не
/// открывается, — тогда UTC системы.
async fn request_time(config: &Config) -> chrono::NaiveDateTime {
    if config.data_dir.join("mywork.db").is_file() {
        match open_core(config).await {
            Ok((core, db)) => {
                let now = core.now();
                drop(core);
                let _ = db.close().await;
                return now;
            }
            Err(e) => tracing::warn!("база не открылась, время запроса — по UTC системы: {e}"),
        }
    } else {
        tracing::warn!("базы нет, время запроса — по UTC системы");
    }
    chrono::Utc::now().naive_utc()
}

/// Подсказка после импорта командой: сервер на SQLite этой папки данных во время импорта не запущен
/// (импорт держит замок папки), а с базой, заданной адресом, может работать сервер с другой
/// папкой данных — у него старый кэш настроек.
fn restart_hint(config: &Config) -> Option<&'static str> {
    config.database_url.is_some().then_some(
        "Если с базой, заданной адресом, уже работает сервер с другой папкой данных, перезапустите его",
    )
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
        let cli = Cli::try_parse_from(["mw-server", "--database-url", "postgres://mw:secret@h/db"])
            .unwrap();
        assert!(!format!("{cli:?}").contains("secret"));
    }

    #[test]
    fn timezone_must_be_an_iana_name() {
        let cli = Cli::try_parse_from(["mw-server", "--timezone", "Moscow"]).unwrap();
        let err = cli.config().unwrap_err().to_string();
        assert!(err.contains("«Moscow»"), "{err}");
        let cli = Cli::try_parse_from(["mw-server", "--timezone", "Europe/Moscow"]).unwrap();
        assert!(cli.config().is_ok());
    }

    #[test]
    fn empty_database_url_is_rejected() {
        let cli = Cli::try_parse_from(["mw-server", "--database-url", ""]).unwrap();
        let err = cli.config().unwrap_err().to_string();
        assert!(err.contains("адрес базы данных пуст"), "{err}");
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

    #[test]
    fn parses_log_dir_and_exit_on_stdin_close() {
        let cli = Cli::try_parse_from(["mw-server"]).unwrap();
        assert!(!cli.exit_on_stdin_close);
        assert_eq!(
            cli.resolve_log_dir(std::path::Path::new("/d")),
            Some(PathBuf::from("/d/logs"))
        );
        let cli = Cli::try_parse_from([
            "mw-server",
            "serve",
            "--log-dir",
            "none",
            "--exit-on-stdin-close",
        ])
        .unwrap();
        assert!(cli.exit_on_stdin_close);
        assert_eq!(cli.resolve_log_dir(std::path::Path::new("/d")), None);
        let cli = Cli::try_parse_from(["mw-server", "--log-dir", "/x"]).unwrap();
        assert_eq!(
            cli.resolve_log_dir(std::path::Path::new("/d")),
            Some(PathBuf::from("/x"))
        );
    }

    #[test]
    fn parses_ui_dir() {
        let cli = Cli::try_parse_from(["mw-server", "--ui-dir", "/ui"]).unwrap();
        assert_eq!(cli.config().unwrap().ui_dir, Some(PathBuf::from("/ui")));
    }

    #[test]
    fn the_restart_hint_is_only_for_a_base_another_data_folder_may_share() {
        let cli = Cli::try_parse_from(["mw-server", "--data-dir", "/d"]).unwrap();
        // SQLite этой папки: сервер на ней не запущен — импорт держит замок папки.
        assert_eq!(restart_hint(&cli.config().unwrap()), None);
        let cli = Cli::try_parse_from([
            "mw-server",
            "--data-dir",
            "/d",
            "--database-url",
            "postgres://db/mw",
        ])
        .unwrap();
        let hint = restart_hint(&cli.config().unwrap()).unwrap();
        assert!(hint.contains("с базой, заданной адресом"), "{hint}");
        assert!(!hint.contains("PostgreSQL"), "{hint}");
    }

    #[test]
    fn parses_import_legacy() {
        let cli = Cli::try_parse_from(["mw-server", "import-legacy", "/old", "--replace"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::ImportLegacy { ref source, replace: true }) if source == std::path::Path::new("/old")
        ));
        let cli = Cli::try_parse_from(["mw-server", "import-legacy", "/old"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::ImportLegacy { replace: false, .. })
        ));
    }
}
