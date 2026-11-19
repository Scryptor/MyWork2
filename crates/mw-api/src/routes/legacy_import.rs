//! Импорт из MyWork 2 из интерфейса: проверка папки, перенос фоновой задачей с ходом по этапам,
//! прошлые импорты с отчётами.
//!
//! Путь к папке — на диске сервера, поэтому проверка и перенос разрешены только серверу на loopback.
//! Права — как у самого импорта (`settings_manage` и `users_manage`, проверяет `mw-core`).

use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use mw_core::{Actor, Core};
use mw_legacy::{
    FolderCheck, ImportJob, ImportOptions, ImportReport, ImportReportView, LegacyError,
    LegacyImportItem, Progress, StartedFrom,
};

use crate::auth::CurrentActor;
use crate::dto::{LegacyImportCheck, LegacyImportStart};
use crate::error::ApiError;
use crate::extract::{Json, Path};
use crate::state::{AppState, ImportJobs};

/// Сервер слушает loopback, у пользователя есть права импорта; возвращает папку данных.
fn local_import_allowed(state: &AppState, actor: &Actor) -> Result<PathBuf, ApiError> {
    if !state.config.loopback() {
        return Err(ApiError::import_local_only());
    }
    state.core.check_import_rights(actor)?;
    state
        .core
        .data_dir_path()
        .map(FsPath::to_path_buf)
        .ok_or_else(ApiError::no_data_dir)
}

/// Проверка папки MyWork 2 без записи: что будет перенесено или что не так
/// (права `settings_manage` и `users_manage`, только сервер на этом компьютере).
#[utoipa::path(
    post,
    path = "/api/v1/legacy-import/check",
    tag = "Импорт из MyWork 2",
    security(("bearer" = [])),
    request_body = LegacyImportCheck,
    responses((status = 200, description = "Итог проверки", body = FolderCheck))
)]
pub(super) async fn check_legacy_folder(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Json(body): Json<LegacyImportCheck>,
) -> Result<Json<FolderCheck>, ApiError> {
    let data_dir = local_import_allowed(&state, &actor)?;
    Ok(Json(
        mw_legacy::check_folder(FsPath::new(&body.path), &data_dir).await,
    ))
}

/// Запуск переноса фоновой задачей; ход — `GET /legacy-import/current`. Одновременно идёт один
/// импорт: второй запуск — 409. Ошибка переноса приходит в ходе (`error`), а не ответом
/// (права `settings_manage` и `users_manage`, только сервер на этом компьютере).
#[utoipa::path(
    post,
    path = "/api/v1/legacy-import",
    tag = "Импорт из MyWork 2",
    security(("bearer" = [])),
    request_body = LegacyImportStart,
    responses((status = 202, description = "Импорт запущен", body = ImportJob))
)]
pub(super) async fn start_legacy_import(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Json(body): Json<LegacyImportStart>,
) -> Result<(StatusCode, Json<ImportJob>), ApiError> {
    let data_dir = local_import_allowed(&state, &actor)?;
    let source = PathBuf::from(&body.path);
    if !source.is_absolute() {
        return Err(ApiError::bad_request(
            "Укажите полный путь к папке MyWork 2",
        ));
    }
    let job = state
        .import
        .try_start()
        .ok_or_else(ApiError::import_running)?;
    tracing::info!(
        replace = body.replace,
        "импорт из MyWork 2 запущен из интерфейса"
    );
    tokio::spawn(run_import(
        state.import.clone(),
        state.core.clone(),
        actor,
        ImportOptions {
            source,
            attachments_dir: data_dir.join("attachments"),
            backups_dir: data_dir.join("backups"),
            replace: body.replace,
            progress: None,
        },
        data_dir,
    ));
    Ok((StatusCode::ACCEPTED, Json(job)))
}

/// Ход текущего или последнего импорта с запуска сервера; `null` — импорта не было
/// (права `settings_manage` и `users_manage`, только сервер на этом компьютере).
#[utoipa::path(
    get,
    path = "/api/v1/legacy-import/current",
    tag = "Импорт из MyWork 2",
    security(("bearer" = [])),
    responses((status = 200, description = "Ход импорта", body = Option<ImportJob>))
)]
pub(super) async fn current_legacy_import(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
) -> Result<Json<Option<ImportJob>>, ApiError> {
    local_import_allowed(&state, &actor)?;
    Ok(Json(state.import.current()))
}

/// Прошлые импорты, новые первыми (права `settings_manage` и `users_manage`).
#[utoipa::path(
    get,
    path = "/api/v1/legacy-imports",
    tag = "Импорт из MyWork 2",
    security(("bearer" = [])),
    responses((status = 200, description = "Прошлые импорты", body = Vec<LegacyImportItem>))
)]
pub(super) async fn list_legacy_imports(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
) -> Result<Json<Vec<LegacyImportItem>>, ApiError> {
    let records = state.core.list_legacy_imports(&actor).await?;
    Ok(Json(records.iter().map(mw_legacy::import_item).collect()))
}

/// Отчёт прошлого импорта: итоги, группы предупреждений с номерами заказов, полный текст
/// (права `settings_manage` и `users_manage`).
#[utoipa::path(
    get,
    path = "/api/v1/legacy-imports/{id}",
    tag = "Импорт из MyWork 2",
    security(("bearer" = [])),
    params(("id" = i64, Path, description = "Номер импорта")),
    responses((status = 200, description = "Отчёт импорта", body = ImportReportView))
)]
pub(super) async fn get_legacy_import(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(id): Path<i64>,
) -> Result<Json<ImportReportView>, ApiError> {
    let record = state.core.legacy_import(&actor, id).await?;
    Ok(Json(mw_legacy::report_view(&record)))
}

/// Если задача импорта оборвалась (паника), ход не должен остаться «идёт» навсегда.
struct FinishOnDrop(Option<Arc<ImportJobs>>);

impl Drop for FinishOnDrop {
    fn drop(&mut self) {
        if let Some(jobs) = self.0.take() {
            jobs.finish(
                None,
                Some("Импорт прерван внутренней ошибкой сервера".into()),
            );
        }
    }
}

/// Фоновая задача: импорт, затем файлы отчёта в `<данные>/import-reports` и итог в `jobs`.
async fn run_import(
    jobs: Arc<ImportJobs>,
    core: Core,
    actor: Actor,
    mut options: ImportOptions,
    data_dir: PathBuf,
) {
    let mut guard = FinishOnDrop(Some(jobs.clone()));
    let progress: Progress = {
        let jobs = jobs.clone();
        Arc::new(move |stage, percent| jobs.progress(stage, percent))
    };
    options.progress = Some(progress);
    let (import_id, error) = match mw_legacy::import_folder(&core, &actor, &options).await {
        Ok(report) => {
            let import_id = report.summary.import_id;
            write_reports(&core, data_dir, report).await;
            (Some(import_id), None)
        }
        Err(error) => {
            let message = mw_legacy::import_error_message(&error, StartedFrom::Interface);
            tracing::warn!("импорт из интерфейса не удался: {message}");
            // База уже записана, файлы остались в промежуточной папке: отчёт всё равно нужен.
            let import_id = match error {
                LegacyError::FilesNotPlaced { report, .. } => {
                    let import_id = report.summary.import_id;
                    write_reports(&core, data_dir, *report).await;
                    Some(import_id)
                }
                _ => None,
            };
            (import_id, Some(message))
        }
    };
    guard.0 = None;
    if error.is_none() {
        tracing::info!(import_id, "импорт из интерфейса завершён");
    }
    jobs.finish(import_id, error);
}

/// Файлы отчёта, как у команды `import-legacy`. Отчёт есть и в базе (`GET /legacy-imports/{id}`),
/// поэтому неудача записи файлов только попадает в лог.
async fn write_reports(core: &Core, data_dir: PathBuf, report: ImportReport) {
    let at = core.now();
    let written =
        tokio::task::spawn_blocking(move || mw_legacy::write_report_files(&data_dir, at, &report))
            .await;
    match written {
        Ok(Ok(_)) => {}
        Ok(Err(e)) => tracing::warn!("файлы отчёта об импорте не записаны: {e}"),
        Err(e) => tracing::warn!("файлы отчёта об импорте не записаны: {e}"),
    }
}
