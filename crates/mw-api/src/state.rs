//! Состояние приложения и настройки HTTP-слоя.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Bytes;
use chrono::Duration;
use mw_core::Core;
use mw_legacy::{ImportJob, ImportStage};

use crate::auth::{ACCOUNT_FAILURE_LIMIT, FAILURE_WINDOW_SECS, IP_FAILURE_LIMIT};
use crate::limiter::LoginLimiter;

/// Настройки HTTP-слоя.
#[derive(Debug, Clone)]
pub struct ApiConfig {
    /// Сервер слушает только loopback-адрес: включает проверку `Host` и режим без входа.
    loopback: bool,
    /// Папка собранного интерфейса; пусто — интерфейс не раздаётся.
    ui_dir: Option<PathBuf>,
}

impl ApiConfig {
    /// Настройки по фактическому адресу, на котором сервер слушает.
    pub fn for_listen_addr(addr: SocketAddr) -> ApiConfig {
        ApiConfig {
            loopback: addr.ip().is_loopback(),
            ui_dir: None,
        }
    }

    /// Раздавать интерфейс из этой папки (в ней должен быть `index.html`).
    pub fn with_ui_dir(mut self, dir: Option<PathBuf>) -> ApiConfig {
        self.ui_dir = dir;
        self
    }

    /// Слушает ли сервер только loopback-адрес.
    pub fn loopback(&self) -> bool {
        self.loopback
    }

    /// Папка интерфейса, если он раздаётся.
    pub fn ui_dir(&self) -> Option<&std::path::Path> {
        self.ui_dir.as_deref()
    }
}

/// Общее состояние обработчиков.
#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) core: Core,
    pub(crate) config: ApiConfig,
    /// Готовый документ OpenAPI (JSON), чтобы не собирать его на каждый запрос.
    pub(crate) openapi_json: Bytes,
    /// Ограничители неудачных попыток входа и смены пароля.
    pub(crate) limits: Arc<Limits>,
    /// Импорт из MyWork 2, запущенный из интерфейса.
    pub(crate) import: Arc<ImportJobs>,
}

/// Импорт из MyWork 2 из интерфейса: одна задача на сервер; последний итог хранится до нового запуска.
#[derive(Default)]
pub(crate) struct ImportJobs {
    current: std::sync::Mutex<Option<ImportJob>>,
}

impl ImportJobs {
    fn lock(&self) -> std::sync::MutexGuard<'_, Option<ImportJob>> {
        self.current.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Ход текущего или последнего импорта.
    pub(crate) fn current(&self) -> Option<ImportJob> {
        self.lock().clone()
    }

    /// Начинает новый импорт; `None`, если прежний ещё идёт.
    pub(crate) fn try_start(&self) -> Option<ImportJob> {
        let mut current = self.lock();
        if current.as_ref().is_some_and(ImportJob::is_running) {
            return None;
        }
        let job = ImportJob::started();
        *current = Some(job.clone());
        Some(job)
    }

    /// Ход по этапам. «Готово» ставит только `finish`: вместе с номером импорта.
    pub(crate) fn progress(&self, stage: ImportStage, percent: u8) {
        if stage == ImportStage::Done {
            return;
        }
        if let Some(job) = self.lock().as_mut().filter(|job| job.is_running()) {
            job.stage = stage;
            job.percent = percent;
        }
    }

    /// Итог: номер записанного импорта и ошибка (её нет — импорт готов).
    pub(crate) fn finish(&self, import_id: Option<i64>, error: Option<String>) {
        if let Some(job) = self.lock().as_mut().filter(|job| job.is_running()) {
            job.import_id = import_id;
            match error {
                Some(error) => job.error = Some(error),
                None => {
                    job.stage = ImportStage::Done;
                    job.percent = 100;
                }
            }
        }
    }
}

/// Ограничители по учётной записи (логину или пользователю) и по адресу клиента.
pub(crate) struct Limits {
    pub(crate) accounts: LoginLimiter,
    pub(crate) ips: LoginLimiter,
}

impl Limits {
    pub(crate) fn new() -> Self {
        let window = Duration::seconds(FAILURE_WINDOW_SECS);
        Limits {
            accounts: LoginLimiter::new(ACCOUNT_FAILURE_LIMIT, window),
            ips: LoginLimiter::new(IP_FAILURE_LIMIT, window),
        }
    }
}
