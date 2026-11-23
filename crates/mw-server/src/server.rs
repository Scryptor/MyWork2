use crate::config::{
    Config, ServerError, redact_database_url, scrub_url, validate_timezone, validate_ui_dir,
};
use axum::Router;
use mw_api::ApiConfig;
use mw_core::documents::code_render_pool;
use mw_core::{Core, SystemClock};
use sea_orm::DatabaseConnection;
use std::future::{Future, IntoFuture};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

/// Запущенный, но ещё не обслуживающий запросы сервер: база подключена и мигрирована, порт занят.
pub struct Server {
    core: Core,
    db: DatabaseConnection,
    listener: TcpListener,
    router: Router,
    /// Сколько после сигнала остановки ждать запросы, которые ещё идут.
    grace: std::time::Duration,
}

/// Подключает базу и применяет миграции; сообщение ошибки не содержит адреса базы.
pub(crate) async fn connect_and_migrate(
    config: &Config,
) -> Result<DatabaseConnection, ServerError> {
    std::fs::create_dir_all(&config.data_dir).map_err(|source| ServerError::Io {
        context: format!(
            "не удалось создать папку данных {}",
            config.data_dir.display()
        ),
        source,
    })?;
    let url = config.database_url();
    tracing::info!("база данных: {}", redact_database_url(&url));
    let db = mw_db::connect(&url).await.map_err(|e| {
        ServerError::Database(format!(
            "не удалось подключиться к базе данных: {}",
            scrub_url(&e.to_string(), &url)
        ))
    })?;
    migrate(&db, Some(&url)).await?;
    Ok(db)
}

async fn migrate(db: &DatabaseConnection, url: Option<&str>) -> Result<(), ServerError> {
    mw_db::migrate(db).await.map_err(|e| {
        let message = e.to_string();
        let message = url.map_or(message.clone(), |url| scrub_url(&message, url));
        ServerError::Database(format!(
            "не удалось применить миграции (возможно, база создана более новой версией программы): {message}"
        ))
    })
}

/// Пишет в лог часовой пояс предприятия и предупреждает, если заданный явно пояс не применён.
pub(crate) fn log_timezone(core: &Core, requested: Option<&str>) {
    let effective = core.settings().timezone;
    tracing::info!("часовой пояс предприятия: {effective}");
    if let Some(requested) = requested
        && requested != effective
    {
        tracing::warn!(
            "часовой пояс «{requested}» не применён: в базе уже сохранён пояс «{effective}», он меняется в настройках программы"
        );
    }
}

pub(crate) fn system_timezone(config: &Config) -> String {
    config
        .timezone
        .clone()
        .or_else(|| iana_time_zone::get_timezone().ok())
        .unwrap_or_else(|| "UTC".to_string())
}

impl Server {
    /// Создаёт папку данных, подключает базу, применяет миграции и занимает порт.
    pub async fn start(config: &Config) -> Result<Server, ServerError> {
        if let Some(dir) = &config.ui_dir {
            validate_ui_dir(dir)?;
        }
        let db = connect_and_migrate(config).await?;
        // Запасные шрифты нужны только настоящему серверу и программе (оба стартуют здесь); тесты на
        // `start_with_db` остаются на встроенных шрифтах.
        mw_docs::configure_fonts(mw_docs::FontSources {
            dirs: vec![config.data_dir.join("fonts")],
            system: true,
        });
        let server = Server::start_with_db(config, db).await?;
        // Книга шрифтов собирается в фоне, чтобы первый документ не ждал её.
        tokio::task::spawn_blocking(mw_docs::warm_fonts);
        Ok(server)
    }

    /// То же на уже подключённой базе (тесты).
    pub async fn start_with_db(
        config: &Config,
        db: DatabaseConnection,
    ) -> Result<Server, ServerError> {
        if let Some(name) = &config.timezone {
            validate_timezone(name)?;
        }
        if let Some(dir) = &config.ui_dir {
            validate_ui_dir(dir)?;
        }
        migrate(&db, None).await?;
        let mut core = Core::open(db.clone(), Arc::new(SystemClock), &system_timezone(config))
            .await?
            .with_attachments_dir(config.data_dir.clone());
        // Шаблоны со своим Typst-кодом строятся процессами-рендерерами; их запасные шрифты — те же,
        // что у сервера.
        let render_pool = config.render_worker.clone().map(|program| {
            code_render_pool(
                program,
                mw_docs::FontSources {
                    dirs: vec![config.data_dir.join("fonts")],
                    system: true,
                },
            )
        });
        if let Some(pool) = &render_pool {
            core = core.with_render_pool(pool.clone());
        }
        finish_interrupted_imports(&core, config).await;
        log_timezone(&core, config.timezone.as_deref());
        let listener =
            TcpListener::bind(config.listen)
                .await
                .map_err(|source| ServerError::Io {
                    context: format!("не удалось занять адрес {}", config.listen),
                    source,
                })?;
        let local_addr = listener.local_addr().map_err(|source| ServerError::Io {
            context: "не удалось узнать адрес сервера".into(),
            source,
        })?;
        let router = mw_api::router(
            core.clone(),
            ApiConfig::for_listen_addr(local_addr).with_ui_dir(config.ui_dir.clone()),
        );
        tracing::info!("сервер слушает {local_addr}");
        if let Some(pool) = render_pool {
            // Один процесс-рендерер запускается заранее, в фоне: запуск сервера его не ждёт (книга
            // шрифтов системы собирается до минуты).
            tokio::task::spawn_blocking(move || pool.warm());
        }
        Ok(Server {
            core,
            db,
            listener,
            router,
            grace: mw_api::REQUEST_TIMEOUT,
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        // Адрес известен с момента привязки; запасной вариант нужен только для типов.
        self.listener
            .local_addr()
            .unwrap_or_else(|_| SocketAddr::from(([127, 0, 0, 1], 0)))
    }

    pub fn core(&self) -> &Core {
        &self.core
    }

    /// Сколько после сигнала остановки ждать запросы, которые ещё идут (по умолчанию —
    /// `mw_api::REQUEST_TIMEOUT`); короче — для тестов.
    pub fn with_shutdown_grace(mut self, grace: std::time::Duration) -> Server {
        self.grace = grace;
        self
    }

    /// Обслуживает запросы до сигнала `shutdown`, затем закрывает пул базы. После сигнала новые
    /// соединения не принимаются, а запросы, которые ещё идут, получают `with_shutdown_grace` (30 с):
    /// иначе остановка ждала бы без конца запрос без ограничения времени (проверку папки MyWork 2), и
    /// закрытая программа держала бы папку данных. Фоновые задачи (`Core::wait_background`) это не
    /// затрагивает.
    pub async fn serve(
        self,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> Result<(), ServerError> {
        let Server {
            core,
            db,
            listener,
            router,
            grace,
        } = self;
        let cleanup = spawn_draft_cleanup(core.clone());
        let (signalled, on_signal) = tokio::sync::oneshot::channel::<()>();
        let serving = axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async move {
            shutdown.await;
            let _ = signalled.send(());
        })
        .into_future();
        let deadline = async move {
            match on_signal.await {
                Ok(()) => tokio::time::sleep(grace).await,
                // Сервер завершился сам, без сигнала.
                Err(_) => std::future::pending().await,
            }
        };
        let result = tokio::select! {
            served = serving => served.map_err(|source| ServerError::Io {
                context: "ошибка сервера".into(),
                source,
            }),
            () = deadline => {
                tracing::warn!(
                    "запросы не закончились за {} с после сигнала остановки: сервер останавливается без них",
                    grace.as_secs()
                );
                Ok(())
            }
        };
        cleanup.abort();
        // Перенос файлов черновиков, начатый до остановки, доходит до конца раньше закрытия БД.
        // Импорт до начала записи в базу отменяется; срок `BACKGROUND_WAIT` действует только на это.
        // Начатую запись импорта остановка дожидается и после срока: `db.close()` ждёт, пока
        // соединение её транзакции вернётся, то есть фиксации.
        core.wait_background(BACKGROUND_WAIT).await;
        drop(core);
        if let Err(e) = db.close().await {
            tracing::warn!("не удалось закрыть соединения с базой: {e}");
        }
        tracing::info!("сервер остановлен");
        result
    }
}

/// Разбирает промежуточные папки оборванного импорта (закрыли программу, упал сервер): файлы импорта,
/// уже записанного в базу, ставятся на место, остальные папки удаляются.
async fn finish_interrupted_imports(core: &Core, config: &Config) {
    let attachments = config.data_dir.join("attachments");
    let cleanup = match mw_legacy::finish_interrupted_imports(core, &attachments).await {
        Ok(cleanup) => cleanup,
        Err(e) => {
            tracing::warn!("промежуточные папки импорта не разобраны: {e}");
            return;
        }
    };
    for dir in &cleanup.placed {
        tracing::info!(
            "файлы прерванного импорта поставлены на место: {}",
            dir.display()
        );
    }
    for dir in &cleanup.removed {
        tracing::info!(
            "удалена промежуточная папка прерванного импорта: {}",
            dir.display()
        );
    }
    for dir in &cleanup.kept {
        tracing::warn!(
            "промежуточная папка импорта оставлена (файлы не поставлены на место): {}",
            dir.display()
        );
    }
}

/// Сколько ждать перенос файлов черновиков при остановке, прежде чем закрыть базу.
const BACKGROUND_WAIT: std::time::Duration = std::time::Duration::from_secs(30);

/// Срок жизни черновика приёма с вложениями.
const DRAFT_TTL_HOURS: i64 = 24;
/// Как часто убирать просроченные черновики.
const DRAFT_CLEANUP_EVERY: std::time::Duration = std::time::Duration::from_secs(3600);

/// Фоновая уборка черновиков: сразу при запуске, затем раз в час; ошибки идут в лог.
pub(crate) fn spawn_draft_cleanup(core: Core) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(DRAFT_CLEANUP_EVERY);
        loop {
            tick.tick().await;
            match core
                .cleanup_drafts(chrono::Duration::hours(DRAFT_TTL_HOURS))
                .await
            {
                Ok(0) => {}
                Ok(n) => tracing::info!("убраны просроченные вложения черновиков: {n}"),
                Err(e) => tracing::warn!("не удалось убрать просроченные черновики: {e}"),
            }
        }
    })
}

/// Ждёт Ctrl+C или (на unix) SIGTERM.
pub async fn shutdown_signal() {
    let ctrl_c = async {
        if tokio::signal::ctrl_c().await.is_err() {
            std::future::pending::<()>().await;
        }
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("получен сигнал остановки");
}
