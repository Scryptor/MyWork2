//! Резервные копии: база и файлы вложений в папке копий, автокопия раз в сутки.

mod copy;
mod settings;

use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;
use mw_domain::permission::Permission;
use mw_domain::validation::FieldError;
use sea_orm::DatabaseBackend;
use serde::{Deserialize, Serialize};
use tokio::sync::OwnedMutexGuard;

#[allow(unused_imports)] // для восстановления (следующая задача)
pub(crate) use copy::{Manifest, list_copies, unique_name, write_manifest};

use crate::{Actor, Core, CoreError, NewAuditEntry, write_audit};

/// Файл настроек в папке данных.
pub const SETTINGS_FILE: &str = "backup-settings.json";
/// Папка копий по умолчанию внутри папки данных.
pub const DEFAULT_DIR: &str = "backups";
/// Описание копии внутри её папки.
pub const MANIFEST_FILE: &str = "mywork-backup.json";
/// Начало имени недоделанной копии.
pub const PARTIAL_PREFIX: &str = ".partial-";
/// Наибольшее число хранимых автокопий.
pub const KEEP_MAX: u16 = 100;

const REASON_POSTGRES: &str =
    "Для PostgreSQL резервные копии делает pg_dump по расписанию на сервере";
const REASON_OTHER: &str = "Резервные копии делаются только для базы в папке данных";
const BUSY: &str = "Копия уже делается — дождитесь её";

/// Вид копии.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum BackupKind {
    Auto,
    Manual,
    BeforeRestore,
}

/// Копия в папке копий.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct BackupCopy {
    /// Имя папки копии.
    pub name: String,
    pub kind: BackupKind,
    /// Когда сделана (местное время предприятия).
    pub created_at: NaiveDateTime,
    /// Версия программы, сделавшей копию.
    pub app_version: String,
    /// Размер базы, байт.
    #[ts(type = "number")]
    pub database_bytes: u64,
    /// Сколько файлов вложений в копии.
    #[ts(type = "number")]
    pub files: u64,
    /// Размер файлов вложений, байт.
    #[ts(type = "number")]
    pub files_bytes: u64,
}

/// Настройки копий.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
#[serde(default)]
pub struct BackupSettings {
    /// Делать автокопию раз в сутки.
    pub auto: bool,
    /// Сколько последних автокопий хранить, 1–100.
    pub keep: u16,
    /// Своя папка копий; пусто — папка по умолчанию.
    #[schema(required = true)]
    pub folder: Option<String>,
    /// Пользователь ответил на предложение взять настройки MyWork 2.
    pub legacy_reviewed: bool,
}

impl Default for BackupSettings {
    fn default() -> Self {
        Self {
            auto: true,
            keep: 10,
            folder: None,
            legacy_reviewed: false,
        }
    }
}

/// Правка настроек; не переданное поле не меняется, `folder: null` возвращает папку по умолчанию.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, ts_rs::TS, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
#[ts(optional_fields)]
pub struct BackupSettingsPatch {
    /// Делать автокопию раз в сутки.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub auto: Option<bool>,
    /// Сколько последних автокопий хранить, 1–100.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub keep: Option<u16>,
    /// Своя папка копий; `null` — папка по умолчанию.
    #[serde(default, deserialize_with = "crate::patch::nullable")]
    pub folder: Option<Option<String>>,
}

/// Ход последней копии с запуска сервера.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct BackupJob {
    pub kind: BackupKind,
    pub started_at: NaiveDateTime,
    /// Когда закончилась; пусто — идёт.
    #[schema(required = true)]
    pub finished_at: Option<NaiveDateTime>,
    /// Имя готовой копии.
    #[schema(required = true)]
    pub copy: Option<String>,
    /// Что пошло не так.
    #[schema(required = true)]
    pub error: Option<String>,
    /// Пометка: например, копия ушла в папку по умолчанию.
    #[schema(required = true)]
    pub note: Option<String>,
}

/// Восстановление, которое ждёт перезапуска программы.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct BackupRestorePending {
    pub copy: BackupCopy,
    pub requested_at: NaiveDateTime,
    pub requested_by: String,
}

/// Итог последнего восстановления.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct BackupRestoreResult {
    pub ok: bool,
    pub copy: BackupCopy,
    pub requested_at: NaiveDateTime,
    /// Имя страховочной копии прежних данных.
    #[schema(required = true)]
    pub safety: Option<String>,
    #[schema(required = true)]
    pub error: Option<String>,
}

/// Настройки копий в MyWork 2, которые можно взять.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct BackupLegacySettings {
    pub auto: bool,
    pub folders: Vec<String>,
}

/// Всё для экрана «Резервные копии».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct BackupOverview {
    pub supported: bool,
    #[schema(required = true)]
    pub reason: Option<String>,
    pub settings: BackupSettings,
    /// Полный путь папки, из которой список.
    pub folder: String,
    /// Почему выбранная папка не используется.
    #[schema(required = true)]
    pub folder_problem: Option<String>,
    /// Новые сверху.
    pub copies: Vec<BackupCopy>,
    #[schema(required = true)]
    pub job: Option<BackupJob>,
    #[schema(required = true)]
    pub pending: Option<BackupRestorePending>,
    #[schema(required = true)]
    pub last_restore: Option<BackupRestoreResult>,
    #[schema(required = true)]
    pub legacy: Option<BackupLegacySettings>,
}

/// Папка по умолчанию.
pub(crate) fn default_backups_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(DEFAULT_DIR)
}

impl Core {
    /// Включает резервные копии: сервер зовёт это, когда база — файл в папке данных.
    pub fn with_backups(mut self) -> Core {
        self.backups_on = true;
        self
    }

    /// Почему копии недоступны; `None` — доступны.
    fn backups_unavailable(&self) -> Option<&'static str> {
        if self.db.get_database_backend() == DatabaseBackend::Postgres {
            Some(REASON_POSTGRES)
        } else if !self.backups_on || self.data_dir.is_none() {
            Some(REASON_OTHER)
        } else {
            None
        }
    }

    /// Папка данных, если копии доступны.
    fn backups_data_dir(&self) -> Result<&Path, CoreError> {
        match (self.backups_unavailable(), self.data_dir.as_deref()) {
            (None, Some(dir)) => Ok(dir),
            (Some(reason), _) => Err(CoreError::Unavailable(reason)),
            (None, None) => Err(CoreError::Unavailable(REASON_OTHER)),
        }
    }

    fn backup_settings_now(&self) -> BackupSettings {
        self.data_dir
            .as_deref()
            .map(settings::read)
            .unwrap_or_default()
    }

    /// Папка, из которой список и в которую идёт автокопия, и почему не выбранная.
    pub(crate) fn backups_root(&self) -> Result<(PathBuf, Option<String>), CoreError> {
        let data_dir = self.backups_data_dir()?;
        let default = default_backups_dir(data_dir);
        let Some(folder) = settings::read(data_dir).folder else {
            return Ok((default, None));
        };
        if Path::new(&folder).is_dir() {
            Ok((PathBuf::from(folder), None))
        } else {
            let problem = format!("Папки {folder} нет — автокопии идут в папку по умолчанию");
            Ok((default, Some(problem)))
        }
    }

    /// Канонический путь папки данных: так копия помечает, чья она.
    fn backup_source(data_dir: &Path) -> String {
        std::fs::canonicalize(data_dir)
            .unwrap_or_else(|_| data_dir.to_path_buf())
            .to_string_lossy()
            .into_owned()
    }

    /// Всё для экрана копий.
    pub async fn backup_overview(&self, actor: &Actor) -> Result<BackupOverview, CoreError> {
        actor.require(Permission::SettingsManage)?;
        let reason = self.backups_unavailable();
        let settings = self.backup_settings_now();
        let job = self
            .backup_job
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let (folder, folder_problem, copies) = match (reason, self.backups_root()) {
            (None, Ok((dir, problem))) => {
                let copies = list_copies(&dir).into_iter().map(|(_, c, _)| c).collect();
                (dir, problem, copies)
            }
            _ => (
                self.data_dir
                    .as_deref()
                    .map(default_backups_dir)
                    .unwrap_or_default(),
                None,
                Vec::new(),
            ),
        };
        Ok(BackupOverview {
            supported: reason.is_none(),
            reason: reason.map(str::to_owned),
            settings,
            folder: folder.to_string_lossy().into_owned(),
            folder_problem,
            copies,
            job,
            pending: None,
            last_restore: None,
            legacy: None,
        })
    }

    /// Меняет настройки копий.
    pub async fn update_backup_settings(
        &self,
        actor: &Actor,
        patch: BackupSettingsPatch,
    ) -> Result<BackupSettings, CoreError> {
        actor.require(Permission::SettingsManage)?;
        let data_dir = self.backups_data_dir()?.to_path_buf();
        let old = settings::read(&data_dir);
        let mut new = old.clone();
        let mut errors = Vec::new();
        if let Some(auto) = patch.auto {
            new.auto = auto;
        }
        if let Some(keep) = patch.keep {
            if (1..=KEEP_MAX).contains(&keep) {
                new.keep = keep;
            } else {
                errors.push(FieldError::new("keep", "out_of_range"));
            }
        }
        if let Some(folder) = patch.folder {
            match folder {
                None => new.folder = None,
                Some(f) => {
                    let f = f.trim().to_owned();
                    if new.folder.as_deref() != Some(f.as_str())
                        && let Err(e) = settings::check_folder(&data_dir, &f)
                    {
                        errors.push(e);
                    }
                    new.folder = Some(f);
                }
            }
        }
        if !errors.is_empty() {
            return Err(CoreError::Validation(errors));
        }
        if new == old {
            return Ok(old);
        }
        let mut changes = Vec::new();
        if new.auto != old.auto {
            changes.push(format!(
                "автокопия {}",
                if new.auto {
                    "включена"
                } else {
                    "выключена"
                }
            ));
        }
        if new.keep != old.keep {
            changes.push(format!("хранить {}", new.keep));
        }
        if new.folder != old.folder {
            changes.push(match &new.folder {
                Some(f) => format!("папка {f}"),
                None => "папка по умолчанию".to_owned(),
            });
        }
        let tx = self.begin_write().await?;
        write_audit(
            &tx,
            self.now(),
            actor,
            NewAuditEntry {
                action: "backup.settings",
                order_id: None,
                client_id: None,
                stock_item_id: None,
                summary: format!("Настройки резервных копий: {}", changes.join(", ")),
                details: Some(serde_json::json!({
                    "auto": new.auto, "keep": new.keep, "folder": new.folder,
                })),
            },
        )
        .await?;
        settings::write(&data_dir, &new)
            .map_err(|e| CoreError::Internal(format!("не удалось записать настройки: {e}")))?;
        tx.commit().await?;
        Ok(new)
    }

    fn backup_guard(&self) -> Result<OwnedMutexGuard<()>, CoreError> {
        self.backup_lock
            .clone()
            .try_lock_owned()
            .map_err(|_| CoreError::InUse(BUSY.to_owned()))
    }

    fn set_job(&self, job: Option<BackupJob>) {
        *self.backup_job.lock().unwrap_or_else(|e| e.into_inner()) = job;
    }

    fn job_begin(&self, kind: BackupKind, note: Option<String>) -> BackupJob {
        let job = BackupJob {
            kind,
            started_at: self.now(),
            finished_at: None,
            copy: None,
            error: None,
            note,
        };
        self.set_job(Some(job.clone()));
        job
    }

    fn job_end(&self, result: &Result<BackupCopy, CoreError>) {
        let mut guard = self.backup_job.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(job) = guard.as_mut() {
            job.finished_at = Some(self.now());
            match result {
                Ok(copy) => job.copy = Some(copy.name.clone()),
                Err(CoreError::Internal(m)) => job.error = Some(m.clone()),
                Err(e) => job.error = Some(e.to_string()),
            }
        }
    }

    /// Ручная копия идёт в выбранную папку: её нужно проверить заново, нет — отказ.
    fn manual_target(&self, data_dir: &Path) -> Result<PathBuf, CoreError> {
        match settings::read(data_dir).folder {
            Some(folder) => {
                settings::check_folder(data_dir, &folder)
                    .map_err(|e| CoreError::Validation(vec![e]))?;
                Ok(PathBuf::from(folder))
            }
            None => Ok(default_backups_dir(data_dir)),
        }
    }

    /// Собирает копию в `dir`; ход и блокировка — на вызывающем.
    async fn build_backup(&self, kind: BackupKind, dir: PathBuf) -> Result<BackupCopy, CoreError> {
        let data_dir = self.backups_data_dir()?.to_path_buf();
        let cancel = self.shutdown_token();
        let source = Self::backup_source(&data_dir);
        let at = self.now();
        let prepared = {
            let dir = dir.clone();
            tokio::task::spawn_blocking(move || {
                std::fs::create_dir_all(&dir)?;
                copy::remove_partials(&dir);
                Ok::<_, std::io::Error>(unique_name(&dir, at))
            })
            .await
        };
        let name = prepared
            .map_err(|e| CoreError::Internal(e.to_string()))?
            .map_err(|e| CoreError::Internal(format!("не удалось подготовить папку копий: {e}")))?;
        let partial = dir.join(format!("{PARTIAL_PREFIX}{name}"));
        if let Err(e) = mw_db::backup::vacuum_into(&self.db, &partial.join("mywork.db")).await {
            copy::remove_dir(&partial);
            return Err(CoreError::Internal(format!(
                "не удалось сделать копию базы: {e}"
            )));
        }
        let plan = copy::BuildPlan {
            dir,
            name,
            at,
            kind,
            data_dir,
            source,
        };
        tokio::task::spawn_blocking(move || copy::finish_copy(&plan, &cancel))
            .await
            .map_err(|e| CoreError::Internal(e.to_string()))?
    }

    /// Копия под блокировкой: ход, сборка, запись в журнал для ручной.
    async fn run_backup(
        &self,
        _guard: &OwnedMutexGuard<()>,
        kind: BackupKind,
        dir: PathBuf,
        actor: Option<&Actor>,
    ) -> Result<BackupCopy, CoreError> {
        let result = async {
            let copy = self.build_backup(kind, dir).await?;
            if let (Some(actor), BackupKind::Manual) = (actor, kind) {
                let tx = self.begin_write().await?;
                write_audit(
                    &tx,
                    self.now(),
                    actor,
                    NewAuditEntry {
                        action: "backup.created",
                        order_id: None,
                        client_id: None,
                        stock_item_id: None,
                        summary: format!("Сделана резервная копия {}", copy.name),
                        details: Some(serde_json::json!({ "name": copy.name })),
                    },
                )
                .await?;
                tx.commit().await?;
            }
            Ok(copy)
        }
        .await;
        self.job_end(&result);
        if let Err(e) = &result {
            tracing::warn!(error = %e, "резервная копия не сделана");
        }
        result
    }

    /// Только для тестов: занимает «одну копию за раз», пока жив возвращённый знак.
    #[cfg(any(test, feature = "testing"))]
    pub fn hold_backup_lock_for_tests(&self) -> Result<Box<dyn std::any::Any + Send>, CoreError> {
        Ok(Box::new(self.backup_guard()?))
    }

    /// Делает копию и ждёт её конца.
    pub async fn create_backup(
        &self,
        actor: &Actor,
        kind: BackupKind,
    ) -> Result<BackupCopy, CoreError> {
        actor.require(Permission::SettingsManage)?;
        let data_dir = self.backups_data_dir()?.to_path_buf();
        let dir = match kind {
            BackupKind::Manual => self.manual_target(&data_dir)?,
            _ => self.backups_root()?.0,
        };
        let guard = self.backup_guard()?;
        self.job_begin(kind, None);
        self.run_backup(&guard, kind, dir, Some(actor)).await
    }

    /// Ручная копия фоном: сразу возвращает ход.
    pub async fn start_backup(&self, actor: &Actor) -> Result<BackupJob, CoreError> {
        actor.require(Permission::SettingsManage)?;
        let data_dir = self.backups_data_dir()?.to_path_buf();
        let dir = self.manual_target(&data_dir)?;
        let guard = self.backup_guard()?;
        let job = self.job_begin(BackupKind::Manual, None);
        let core = self.clone();
        let actor = actor.clone();
        self.spawn_background(async move {
            core.run_backup(&guard, BackupKind::Manual, dir, Some(&actor))
                .await
                .ok();
        });
        Ok(job)
    }

    /// Автокопия, если она нужна: включена, не идёт другая и сегодня своей ещё нет.
    pub async fn run_auto_backup_if_due(&self) -> Result<Option<BackupCopy>, CoreError> {
        let Ok(data_dir) = self.backups_data_dir() else {
            return Ok(None);
        };
        let data_dir = data_dir.to_path_buf();
        let settings = settings::read(&data_dir);
        if !settings.auto {
            return Ok(None);
        }
        let Ok(guard) = self.backup_guard() else {
            return Ok(None);
        };
        let (dir, problem) = self.backups_root()?;
        let source = Self::backup_source(&data_dir);
        let today = self.now().date();
        let done = list_copies(&dir).iter().any(|(_, c, m)| {
            c.kind == BackupKind::Auto && m.source == source && c.created_at.date() == today
        });
        if done {
            return Ok(None);
        }
        self.job_begin(
            BackupKind::Auto,
            problem.map(|p| {
                // «Папки X нет — автокопии идут…» → пометка про эту копию.
                p.replace("автокопии идут", "копия сохранена")
            }),
        );
        let copy = self
            .run_backup(&guard, BackupKind::Auto, dir.clone(), None)
            .await?;
        let keep = usize::from(settings.keep);
        let prune_dir = dir;
        tokio::task::spawn_blocking(move || copy::prune_auto(&prune_dir, &source, keep))
            .await
            .map_err(|e| CoreError::Internal(e.to_string()))?;
        Ok(Some(copy))
    }

    /// Удаляет копию по имени.
    pub async fn delete_backup(&self, actor: &Actor, name: &str) -> Result<(), CoreError> {
        actor.require(Permission::UsersManage)?;
        actor.require(Permission::SettingsManage)?;
        self.backups_data_dir()?;
        let (dir, _) = self.backups_root()?;
        let Some((path, _, _)) = list_copies(&dir)
            .into_iter()
            .find(|(_, c, _)| c.name == name)
        else {
            return Err(CoreError::Validation(vec![FieldError::new(
                "name",
                "backup_missing",
            )]));
        };
        let _guard = self.backup_guard()?;
        tokio::task::spawn_blocking(move || copy::remove_dir_checked(&path))
            .await
            .map_err(|e| CoreError::Internal(e.to_string()))?
            .map_err(|e| CoreError::Internal(format!("не удалось удалить копию: {e}")))?;
        let tx = self.begin_write().await?;
        write_audit(
            &tx,
            self.now(),
            actor,
            NewAuditEntry {
                action: "backup.deleted",
                order_id: None,
                client_id: None,
                stock_item_id: None,
                summary: format!("Удалена резервная копия {name}"),
                details: Some(serde_json::json!({ "name": name })),
            },
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
}
