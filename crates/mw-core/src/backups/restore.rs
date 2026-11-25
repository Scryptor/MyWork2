//! Восстановление из копии: запрос и отмена в работающей программе, шаги — при следующем запуске
//! до подключения базы (`apply_pending_restore` работает только с файлами).

use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;
use mw_domain::permission::Permission;
use mw_domain::validation::FieldError;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use super::copy::{MANIFEST_FORMAT, Manifest, remove_dir_checked, unique_name, write_manifest};
use super::{
    BackupCopy, BackupKind, BackupRestorePending, BackupRestoreResult, MANIFEST_FILE,
    default_backups_dir,
};
use crate::attachments::tree::copy_tree;
use crate::{Actor, Core, CoreError, NewAuditEntry, write_audit};

/// Отметка «восстановить при запуске» в папке данных.
pub const RESTORE_MARKER: &str = "restore.json";
/// Рабочая папка восстановления в папке данных.
pub const RESTORE_STAGING: &str = ".restore";
/// Итог последнего восстановления в папке данных.
pub const RESTORE_RESULT: &str = "restore-result.json";

const PHASE_FILE: &str = "phase.json";
const PHASE_COPIED: &str = "copied";
const PHASE_MOVED_ASIDE: &str = "moved_aside";
const DB_FILE: &str = "mywork.db";
const DB_WAL: &str = "mywork.db-wal";
const DB_SHM: &str = "mywork.db-shm";
const ATTACHMENTS: &str = "attachments";

/// Отметка в `restore.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Marker {
    /// Полный путь папки копии.
    copy_path: PathBuf,
    copy: BackupCopy,
    requested_at: NaiveDateTime,
    requested_by: String,
}

/// Шаг в `.restore/phase.json`.
#[derive(Debug, Serialize, Deserialize)]
struct Phase {
    phase: String,
    /// Имя страховочной копии.
    safety: String,
}

/// Где остановить восстановление (только для тестов): сразу после записи шага.
#[cfg(any(test, feature = "testing"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreStop {
    AfterCopied,
    AfterMovedAside,
}

fn io_err(what: &str, path: &Path, e: std::io::Error) -> CoreError {
    CoreError::Internal(format!("{what}: {} ({e})", path.display()))
}

/// Читает JSON-файл; нет файла — `None`, испорчен — `None` и предупреждение в журнале работы.
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    let bytes = std::fs::read(path).ok()?;
    match serde_json::from_slice(&bytes) {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "файл восстановления не разобран, считается отсутствующим");
            None
        }
    }
}

/// Пишет JSON во временный файл рядом и переименовывает его.
fn write_json<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    let bytes = serde_json::to_vec_pretty(value).map_err(std::io::Error::other)?;
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        std::fs::remove_file(&tmp).ok();
    })
}

fn read_marker(data_dir: &Path) -> Option<Marker> {
    read_json(&data_dir.join(RESTORE_MARKER))
}

/// Ждущее восстановление.
pub(crate) fn read_pending(data_dir: &Path) -> Option<BackupRestorePending> {
    read_marker(data_dir).map(|m| BackupRestorePending {
        copy: m.copy,
        requested_at: m.requested_at,
        requested_by: m.requested_by,
    })
}

/// Путь копии, которая ждёт восстановления.
pub(crate) fn pending_copy_path(data_dir: &Path) -> Option<PathBuf> {
    read_marker(data_dir).map(|m| m.copy_path)
}

/// Итог последнего восстановления.
pub(crate) fn read_last_result(data_dir: &Path) -> Option<BackupRestoreResult> {
    read_json(&data_dir.join(RESTORE_RESULT))
}

fn bad(code: &str) -> CoreError {
    CoreError::Validation(vec![FieldError::new("name", code)])
}

/// Описание копии из её папки; нет или не разобрано — «копии нет».
fn read_manifest(copy_dir: &Path) -> Option<Manifest> {
    let bytes = std::fs::read(copy_dir.join(MANIFEST_FILE)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Годится ли файл базы: цел и без неизвестных миграций. `Err` — код ошибки поля `name`.
async fn check_base(db: &Path) -> Result<(), &'static str> {
    match mw_db::backup::inspect_copy(db).await {
        Err(_) => Err("backup_damaged"),
        Ok(c) if !c.intact => Err("backup_damaged"),
        Ok(c) if !c.unknown_migrations.is_empty() => Err("backup_too_new"),
        Ok(_) => Ok(()),
    }
}

/// Проверки копии: есть, формат известен, база цела и не от новой версии.
async fn check_copy(copy_dir: &Path) -> Result<BackupCopy, &'static str> {
    let manifest = read_manifest(copy_dir).ok_or("backup_missing")?;
    if manifest.format > MANIFEST_FORMAT {
        return Err("backup_too_new");
    }
    check_base(&copy_dir.join(DB_FILE)).await?;
    let name = copy_dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(manifest.to_copy(&name))
}

/// Проверяет копию и пишет отметку `restore.json`; без `Core`, для запроса и командной строки.
pub async fn schedule_restore(
    data_dir: &Path,
    copy_dir: &Path,
    requested_at: NaiveDateTime,
    requested_by: &str,
) -> Result<BackupRestorePending, CoreError> {
    let copy = check_copy(copy_dir).await.map_err(bad)?;
    write_marker(data_dir, copy_dir, &copy, requested_at, requested_by)
}

fn write_marker(
    data_dir: &Path,
    copy_dir: &Path,
    copy: &BackupCopy,
    requested_at: NaiveDateTime,
    requested_by: &str,
) -> Result<BackupRestorePending, CoreError> {
    let marker = Marker {
        copy_path: copy_dir.to_path_buf(),
        copy: copy.clone(),
        requested_at,
        requested_by: requested_by.to_owned(),
    };
    let path = data_dir.join(RESTORE_MARKER);
    write_json(&path, &marker)
        .map_err(|e| io_err("не удалось записать отметку восстановления", &path, e))?;
    Ok(BackupRestorePending {
        copy: marker.copy,
        requested_at,
        requested_by: marker.requested_by,
    })
}

impl Core {
    /// Просит восстановить данные из копии при следующем запуске программы.
    pub async fn request_restore(
        &self,
        actor: &Actor,
        name: &str,
    ) -> Result<BackupRestorePending, CoreError> {
        actor.require(Permission::UsersManage)?;
        actor.require(Permission::SettingsManage)?;
        let data_dir = self.backups_data_dir()?.to_path_buf();
        let (dir, _) = self.backups_root()?;
        let Some(path) = super::list_copies(&dir)
            .into_iter()
            .find(|(_, c, _)| c.name == name)
            .map(|(path, _, _)| path)
        else {
            return Err(bad("backup_missing"));
        };
        let _guard = self.backup_guard()?;
        let copy = check_copy(&path).await.map_err(bad)?;
        let at = self.now();
        let tx = self.begin_write().await?;
        write_audit(
            &tx,
            at,
            actor,
            NewAuditEntry {
                action: "backup.restore_requested",
                order_id: None,
                client_id: None,
                stock_item_id: None,
                summary: format!("Запрошено восстановление из копии {name}"),
                details: Some(serde_json::json!({ "name": name })),
            },
        )
        .await?;
        let pending = write_marker(&data_dir, &path, &copy, at, &actor.name)?;
        tx.commit().await?;
        Ok(pending)
    }

    /// Отменяет ждущее восстановление и убирает его итог.
    pub async fn cancel_restore(&self, actor: &Actor) -> Result<(), CoreError> {
        actor.require(Permission::UsersManage)?;
        actor.require(Permission::SettingsManage)?;
        let data_dir = self.backups_data_dir()?.to_path_buf();
        let pending = read_pending(&data_dir);
        let tx = self.begin_write().await?;
        if let Some(pending) = &pending {
            write_audit(
                &tx,
                self.now(),
                actor,
                NewAuditEntry {
                    action: "backup.restore_cancelled",
                    order_id: None,
                    client_id: None,
                    stock_item_id: None,
                    summary: format!("Отменено восстановление из копии {}", pending.copy.name),
                    details: Some(serde_json::json!({ "name": pending.copy.name })),
                },
            )
            .await?;
        }
        for file in [RESTORE_MARKER, RESTORE_RESULT] {
            let path = data_dir.join(file);
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(io_err("не удалось убрать файл", &path, e)),
            }
        }
        tx.commit().await?;
        Ok(())
    }

    /// Пишет в журнал действий итог восстановления: от имени того, кто его запросил.
    pub async fn record_restore(&self, result: &BackupRestoreResult) -> Result<(), CoreError> {
        if !result.ok {
            return Ok(());
        }
        let actor = Actor::unrestricted(result.requested_by.clone());
        let tx = self.begin_write().await?;
        write_audit(
            &tx,
            self.now(),
            &actor,
            NewAuditEntry {
                action: "backup.restored",
                order_id: None,
                client_id: None,
                stock_item_id: None,
                summary: format!(
                    "Данные восстановлены из копии от {}",
                    result.copy.created_at.format("%d.%m.%Y %H:%M")
                ),
                details: Some(serde_json::json!({
                    "name": result.copy.name, "safety": result.safety,
                })),
            },
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
}

/// Выполняет ждущее восстановление (решение 6 плана) до подключения базы. `None` — отметки нет.
pub async fn apply_pending_restore(
    data_dir: &Path,
) -> Result<Option<BackupRestoreResult>, CoreError> {
    apply(data_dir, None).await
}

/// То же, но с остановкой ошибкой сразу после записи шага. Только для тестов.
#[cfg(any(test, feature = "testing"))]
pub async fn apply_pending_restore_until(
    data_dir: &Path,
    stop: RestoreStop,
) -> Result<Option<BackupRestoreResult>, CoreError> {
    let phase = match stop {
        RestoreStop::AfterCopied => PHASE_COPIED,
        RestoreStop::AfterMovedAside => PHASE_MOVED_ASIDE,
    };
    apply(data_dir, Some(phase)).await
}

fn stopped(phase: &str) -> CoreError {
    CoreError::Internal(format!("восстановление остановлено после шага {phase}"))
}

async fn apply(
    data_dir: &Path,
    stop_after: Option<&str>,
) -> Result<Option<BackupRestoreResult>, CoreError> {
    let Some(marker) = read_marker(data_dir) else {
        return Ok(None);
    };
    let staging = data_dir.join(RESTORE_STAGING);
    let mut phase = read_json::<Phase>(&staging.join(PHASE_FILE));

    if phase.is_none() {
        match prepare(data_dir, &marker, &staging).await {
            Ok(safety) => {
                let p = Phase {
                    phase: PHASE_COPIED.to_owned(),
                    safety,
                };
                write_phase(&staging, &p)?;
                phase = Some(p);
                if stop_after == Some(PHASE_COPIED) {
                    return Err(stopped(PHASE_COPIED));
                }
            }
            Err(message) => return fail(data_dir, &marker, &staging, message).map(Some),
        }
    }
    let mut phase = phase.ok_or_else(|| CoreError::Internal("нет шага восстановления".into()))?;

    if phase.phase == PHASE_COPIED {
        let (d, s, m, safety) = (
            data_dir.to_path_buf(),
            staging.clone(),
            marker.clone(),
            phase.safety.clone(),
        );
        run_blocking(move || move_aside(&d, &s, &m, &safety)).await?;
        phase.phase = PHASE_MOVED_ASIDE.to_owned();
        write_phase(&staging, &phase)?;
        if stop_after == Some(PHASE_MOVED_ASIDE) {
            return Err(stopped(PHASE_MOVED_ASIDE));
        }
    }

    let (d, s) = (data_dir.to_path_buf(), staging.clone());
    run_blocking(move || move_into_place(&d, &s)).await?;
    let result = BackupRestoreResult {
        ok: true,
        copy: marker.copy.clone(),
        requested_at: marker.requested_at,
        requested_by: marker.requested_by.clone(),
        safety: Some(phase.safety),
        error: None,
    };
    // Итог — до удаления отметки: оборвавшись между ними, следующий запуск дойдёт до конца сам.
    let result_path = data_dir.join(RESTORE_RESULT);
    write_json(&result_path, &result)
        .map_err(|e| io_err("не удалось записать итог восстановления", &result_path, e))?;
    let marker_path = data_dir.join(RESTORE_MARKER);
    std::fs::remove_file(&marker_path)
        .map_err(|e| io_err("не удалось убрать отметку восстановления", &marker_path, e))?;
    remove_dir_checked(&staging).ok();
    Ok(Some(result))
}

async fn run_blocking<F>(f: F) -> Result<(), CoreError>
where
    F: FnOnce() -> Result<(), CoreError> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| CoreError::Internal(e.to_string()))?
}

fn write_phase(staging: &Path, phase: &Phase) -> Result<(), CoreError> {
    let path = staging.join(PHASE_FILE);
    write_json(&path, phase).map_err(|e| io_err("не удалось записать шаг восстановления", &path, e))
}

/// Ошибка до переноса текущих данных: итог с ошибкой, отметка и `.restore/` убраны.
fn fail(
    data_dir: &Path,
    marker: &Marker,
    staging: &Path,
    message: String,
) -> Result<BackupRestoreResult, CoreError> {
    tracing::warn!(error = %message, "восстановление не выполнено, данные прежние");
    let result = BackupRestoreResult {
        ok: false,
        copy: marker.copy.clone(),
        requested_at: marker.requested_at,
        requested_by: marker.requested_by.clone(),
        safety: None,
        error: Some(message),
    };
    let result_path = data_dir.join(RESTORE_RESULT);
    write_json(&result_path, &result)
        .map_err(|e| io_err("не удалось записать итог восстановления", &result_path, e))?;
    let marker_path = data_dir.join(RESTORE_MARKER);
    std::fs::remove_file(&marker_path)
        .map_err(|e| io_err("не удалось убрать отметку восстановления", &marker_path, e))?;
    remove_dir_checked(staging).ok();
    Ok(result)
}

/// Шаг 1: копирует базу и вложения копии в `.restore/`, проверяет их, выбирает имя страховочной
/// копии. `Err` — текст для итога.
async fn prepare(data_dir: &Path, marker: &Marker, staging: &Path) -> Result<String, String> {
    let code_text = |code: &str| FieldError::new("name", code).message_ru().to_owned();
    let copy_dir = &marker.copy_path;
    match check_copy(copy_dir).await {
        Ok(_) => {}
        Err("backup_missing") => {
            return Err(format!("Копии больше нет: {}", copy_dir.display()));
        }
        Err(code) => return Err(code_text(code)),
    }
    let (src, dst) = (copy_dir.clone(), staging.to_path_buf());
    tokio::task::spawn_blocking(move || copy_into_staging(&src, &dst))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("Не удалось подготовить копию к восстановлению: {e}"))?;
    if let Err(code) = check_base(&staging.join(DB_FILE)).await {
        return Err(code_text(code));
    }
    Ok(unique_name(
        &default_backups_dir(data_dir),
        marker.requested_at,
    ))
}

fn copy_into_staging(copy_dir: &Path, staging: &Path) -> std::io::Result<()> {
    remove_dir_checked(staging)?;
    std::fs::create_dir_all(staging)?;
    std::fs::copy(copy_dir.join(DB_FILE), staging.join(DB_FILE))?;
    let wal = copy_dir.join(DB_WAL);
    if wal.is_file() {
        std::fs::copy(wal, staging.join(DB_WAL))?;
    }
    copy_tree(
        &copy_dir.join(ATTACHMENTS),
        &staging.join(ATTACHMENTS),
        None,
        &CancellationToken::new(),
    )?;
    Ok(())
}

fn exists(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

/// Шаг 2: прежние база и вложения уходят в страховочную копию; переносится только то, что ещё на месте.
fn move_aside(
    data_dir: &Path,
    staging: &Path,
    marker: &Marker,
    safety: &str,
) -> Result<(), CoreError> {
    let safety_dir = default_backups_dir(data_dir).join(safety);
    let fail = |e: std::io::Error| {
        CoreError::Internal(format!(
            "Восстановление не закончено: не удалось сохранить прежние данные в {} ({e}). \
             Копия подготовлена в {}; следующий запуск продолжит.",
            safety_dir.display(),
            staging.display()
        ))
    };
    std::fs::create_dir_all(&safety_dir).map_err(fail)?;
    for name in [DB_FILE, DB_WAL, DB_SHM, ATTACHMENTS] {
        let from = data_dir.join(name);
        if exists(&from) {
            std::fs::rename(&from, safety_dir.join(name)).map_err(fail)?;
        }
    }
    let (files, files_bytes) = tree_stats(&safety_dir.join(ATTACHMENTS));
    let manifest = Manifest {
        format: MANIFEST_FORMAT,
        kind: BackupKind::BeforeRestore,
        created_at: marker.requested_at,
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        source: std::fs::canonicalize(data_dir)
            .unwrap_or_else(|_| data_dir.to_path_buf())
            .to_string_lossy()
            .into_owned(),
        database_bytes: std::fs::metadata(safety_dir.join(DB_FILE)).map_or(0, |m| m.len()),
        files,
        files_bytes,
    };
    write_manifest(&safety_dir, &manifest).map_err(fail)
}

/// Файлов и байт в дереве (ссылки не обходятся).
fn tree_stats(dir: &Path) -> (u64, u64) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return (0, 0);
    };
    let (mut files, mut bytes) = (0, 0);
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            let (f, b) = tree_stats(&entry.path());
            files += f;
            bytes += b;
        } else if kind.is_file() {
            files += 1;
            bytes += entry.metadata().map_or(0, |m| m.len());
        }
    }
    (files, bytes)
}

/// Шаг 3: копия из `.restore/` встаёт на место; вложения раньше базы, база — последней.
fn move_into_place(data_dir: &Path, staging: &Path) -> Result<(), CoreError> {
    for name in [ATTACHMENTS, DB_WAL, DB_FILE] {
        let from = staging.join(name);
        if exists(&from) {
            let to = data_dir.join(name);
            std::fs::rename(&from, &to).map_err(|e| {
                CoreError::Internal(format!(
                    "Восстановление не закончено: не удалось перенести {} на место ({e}). \
                     Следующий запуск продолжит.",
                    from.display()
                ))
            })?;
        }
    }
    Ok(())
}
