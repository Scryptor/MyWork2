//! Резервные копии: список и настройки, ручная копия, удаление, восстановление при следующем
//! запуске, настройки копий MyWork 2.
//!
//! Папка копий лежит на диске сервера, поэтому все маршруты — только серверу на loopback. Права
//! (`settings_manage`; восстановление и удаление — ещё `users_manage`) проверяет `mw-core`.

use axum::extract::State;
use axum::http::StatusCode;
use mw_core::{
    BackupJob, BackupLegacyChoice, BackupOverview, BackupRestorePending, BackupRestoreRequest,
    BackupSettings, BackupSettingsPatch,
};

use crate::auth::CurrentActor;
use crate::error::ApiError;
use crate::extract::{Json, Path};
use crate::state::AppState;

/// Копиями управляют только на компьютере, где работает сервер.
fn local_only(state: &AppState) -> Result<(), ApiError> {
    if state.config.loopback() {
        Ok(())
    } else {
        Err(ApiError::backups_local_only())
    }
}

/// Экран копий: настройки, копии в папке (новые сверху), ход последней копии, ждущее
/// восстановление, итог прошлого восстановления и предложение взять настройки MyWork 2. Для
/// PostgreSQL и базы по адресу `supported: false` с причиной (право `settings_manage`).
#[utoipa::path(
    get,
    path = "/api/v1/backups",
    tag = "Резервные копии",
    security(("bearer" = [])),
    responses((status = 200, description = "Копии и настройки", body = BackupOverview))
)]
pub(super) async fn backup_overview(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
) -> Result<Json<BackupOverview>, ApiError> {
    local_only(&state)?;
    Ok(Json(state.core.backup_overview(&actor).await?))
}

/// Правка настроек копий. Неверное число копий или папка — 422 с полем `keep` или `folder`
/// (право `settings_manage`).
#[utoipa::path(
    patch,
    path = "/api/v1/backups/settings",
    tag = "Резервные копии",
    security(("bearer" = [])),
    request_body = BackupSettingsPatch,
    responses((status = 200, description = "Настройки после правки", body = BackupSettings))
)]
pub(super) async fn update_backup_settings(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Json(patch): Json<BackupSettingsPatch>,
) -> Result<Json<BackupSettings>, ApiError> {
    local_only(&state)?;
    Ok(Json(
        state.core.update_backup_settings(&actor, patch).await?,
    ))
}

/// Ручная копия фоном; ход — в `job` ответа `GET /backups`. Копия уже делается — 409 (право
/// `settings_manage`).
#[utoipa::path(
    post,
    path = "/api/v1/backups",
    tag = "Резервные копии",
    security(("bearer" = [])),
    responses((status = 202, description = "Копия начата", body = BackupJob))
)]
pub(super) async fn start_backup(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
) -> Result<(StatusCode, Json<BackupJob>), ApiError> {
    local_only(&state)?;
    Ok((
        StatusCode::ACCEPTED,
        Json(state.core.start_backup(&actor).await?),
    ))
}

/// Удаление копии по имени папки. Нет такой копии — 422 `backup_missing`; копия ждёт
/// восстановления — 422 `backup_pending` (права `settings_manage` и `users_manage`).
#[utoipa::path(
    delete,
    path = "/api/v1/backups/{name}",
    tag = "Резервные копии",
    security(("bearer" = [])),
    params(("name" = String, Path, description = "Имя папки копии")),
    responses((status = 204, description = "Копия удалена"))
)]
pub(super) async fn delete_backup(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Path(name): Path<String>,
) -> Result<StatusCode, ApiError> {
    local_only(&state)?;
    state.core.delete_backup(&actor, &name).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Просьба восстановить данные из копии при следующем запуске программы; сами данные не меняются
/// до перезапуска (права `settings_manage` и `users_manage`).
#[utoipa::path(
    post,
    path = "/api/v1/backups/restore",
    tag = "Резервные копии",
    security(("bearer" = [])),
    request_body = BackupRestoreRequest,
    responses((status = 202, description = "Восстановление ждёт перезапуска", body = BackupRestorePending))
)]
pub(super) async fn request_restore(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Json(body): Json<BackupRestoreRequest>,
) -> Result<(StatusCode, Json<BackupRestorePending>), ApiError> {
    local_only(&state)?;
    Ok((
        StatusCode::ACCEPTED,
        Json(state.core.request_restore(&actor, &body.name).await?),
    ))
}

/// Отмена ждущего восстановления; ждущего нет — тоже 204 (права `settings_manage` и
/// `users_manage`).
#[utoipa::path(
    delete,
    path = "/api/v1/backups/restore",
    tag = "Резервные копии",
    security(("bearer" = [])),
    responses((status = 204, description = "Восстановление отменено"))
)]
pub(super) async fn cancel_restore(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
) -> Result<StatusCode, ApiError> {
    local_only(&state)?;
    state.core.cancel_restore(&actor).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Ответ на предложение взять настройки копий MyWork 2: `apply: true` берёт автокопию и первую
/// подходящую папку, `false` оставляет свои; предложение больше не показывается (право
/// `settings_manage`).
#[utoipa::path(
    post,
    path = "/api/v1/backups/legacy",
    tag = "Резервные копии",
    security(("bearer" = [])),
    request_body = BackupLegacyChoice,
    responses((status = 200, description = "Настройки после ответа", body = BackupSettings))
)]
pub(super) async fn choose_legacy_backup(
    State(state): State<AppState>,
    CurrentActor(actor): CurrentActor,
    Json(body): Json<BackupLegacyChoice>,
) -> Result<Json<BackupSettings>, ApiError> {
    local_only(&state)?;
    Ok(Json(
        state.core.choose_legacy_backup(&actor, body.apply).await?,
    ))
}
