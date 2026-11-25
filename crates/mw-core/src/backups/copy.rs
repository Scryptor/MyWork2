//! Сборка копии: описание, список, хранение последних N, удаление.

use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use super::{BackupCopy, BackupKind, MANIFEST_FILE, PARTIAL_PREFIX};
use crate::CoreError;
use crate::attachments::tree::copy_tree;

/// Версия формата описания.
pub(crate) const MANIFEST_FORMAT: u32 = 1;

/// Описание копии, `mywork-backup.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Manifest {
    pub format: u32,
    pub kind: BackupKind,
    pub created_at: NaiveDateTime,
    pub app_version: String,
    /// Канонический путь папки данных, из которой сделана копия.
    pub source: String,
    pub database_bytes: u64,
    pub files: u64,
    pub files_bytes: u64,
}

impl Manifest {
    pub(crate) fn to_copy(&self, name: &str) -> BackupCopy {
        BackupCopy {
            name: name.to_owned(),
            kind: self.kind,
            created_at: self.created_at,
            app_version: self.app_version.clone(),
            database_bytes: self.database_bytes,
            files: self.files,
            files_bytes: self.files_bytes,
        }
    }
}

pub(crate) fn write_manifest(dir: &Path, manifest: &Manifest) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(manifest).map_err(std::io::Error::other)?;
    std::fs::write(dir.join(MANIFEST_FILE), bytes)
}

/// Копии папки: только настоящие папки с понятным описанием; новые сверху.
pub(crate) fn list_copies(dir: &Path) -> Vec<(PathBuf, BackupCopy, Manifest)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        if !kind.is_dir() || name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let Ok(bytes) = std::fs::read(path.join(MANIFEST_FILE)) else {
            continue;
        };
        let Ok(manifest) = serde_json::from_slice::<Manifest>(&bytes) else {
            continue;
        };
        out.push((path, manifest.to_copy(&name), manifest));
    }
    out.sort_by(|a, b| (b.1.created_at, &b.1.name).cmp(&(a.1.created_at, &a.1.name)));
    out
}

/// Имя папки копии по времени; занято — `-2`, `-3`.
pub(crate) fn unique_name(dir: &Path, at: NaiveDateTime) -> String {
    let base = at.format("%Y%m%d-%H%M%S").to_string();
    let taken = |n: &str| std::fs::symlink_metadata(dir.join(n)).is_ok();
    if !taken(&base) {
        return base;
    }
    (2..)
        .map(|i| format!("{base}-{i}"))
        .find(|n| !taken(n))
        .unwrap_or(base)
}

/// Убирает недоделанные копии (`.partial-*`).
pub(crate) fn remove_partials(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if entry
            .file_name()
            .to_string_lossy()
            .starts_with(PARTIAL_PREFIX)
        {
            remove_dir(&entry.path());
        }
    }
}

/// Удаляет папку; «уже нет» не ошибка.
pub(crate) fn remove_dir_checked(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_dir_all(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// Удаляет папку, как получится: ошибка только в журнал работы (недоделанные копии, хранение N).
pub(crate) fn remove_dir(path: &Path) {
    if let Err(e) = remove_dir_checked(path) {
        tracing::warn!(path = %path.display(), error = %e, "папка копии не удалена");
    }
}

/// Что нужно, чтобы собрать копию.
pub(crate) struct BuildPlan {
    pub dir: PathBuf,
    pub name: String,
    pub at: NaiveDateTime,
    pub kind: BackupKind,
    pub data_dir: PathBuf,
    pub source: String,
}

/// Файловая часть копии после снимка базы: файлы, описание, переименование `.partial-…`.
/// Ошибка убирает `.partial-…`.
pub(crate) fn finish_copy(
    plan: &BuildPlan,
    cancel: &CancellationToken,
) -> Result<BackupCopy, CoreError> {
    let partial = plan.dir.join(format!("{PARTIAL_PREFIX}{}", plan.name));
    let result = (|| {
        let link_from = list_copies(&plan.dir)
            .into_iter()
            .find(|(path, _, m)| m.source == plan.source && path.join("attachments").is_dir())
            .map(|(path, _, _)| path.join("attachments"));
        let stats = copy_tree(
            &plan.data_dir.join("attachments"),
            &partial.join("attachments"),
            link_from.as_deref(),
            cancel,
        )?;
        let manifest = Manifest {
            format: MANIFEST_FORMAT,
            kind: plan.kind,
            created_at: plan.at,
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            source: plan.source.clone(),
            database_bytes: std::fs::metadata(partial.join("mywork.db"))?.len(),
            files: stats.files,
            files_bytes: stats.bytes,
        };
        write_manifest(&partial, &manifest)?;
        std::fs::rename(&partial, plan.dir.join(&plan.name))?;
        Ok::<_, std::io::Error>(manifest.to_copy(&plan.name))
    })();
    result.map_err(|e| {
        remove_dir(&partial);
        if e.kind() == std::io::ErrorKind::Interrupted {
            CoreError::Internal("Копия прервана: программа закрывается".to_owned())
        } else {
            CoreError::Internal(format!("не удалось сделать копию: {e}"))
        }
    })
}

/// Оставляет `keep` последних автокопий этой папки данных; остальное не трогает.
pub(crate) fn prune_auto(dir: &Path, source: &str, keep: usize) {
    let own_auto = list_copies(dir)
        .into_iter()
        .filter(|(_, c, m)| c.kind == BackupKind::Auto && m.source == source);
    for (path, _, _) in own_auto.skip(keep) {
        remove_dir(&path);
    }
}
