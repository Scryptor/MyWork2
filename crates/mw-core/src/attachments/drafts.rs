//! Черновики приёма: вложения до создания заказа, их перенос в заказ и очистка.

use std::path::PathBuf;

use chrono::Duration;
use mw_db::entity::attachment;
use mw_domain::permission::Permission;
use mw_domain::validation::FieldError;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set,
};

use super::{Attachment, store, to_api};
use crate::{Actor, Core, CoreError};

/// Больше вложений в одном черновике нельзя.
pub const MAX_DRAFT_ATTACHMENTS: u64 = 10;

/// Ключ черновика или повтора приёма: 1–64 символа `[A-Za-z0-9-]`.
pub(crate) fn valid_key(key: &str) -> bool {
    (1..=64).contains(&key.len()) && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn checked_key(field: &'static str, key: &str) -> Result<(), CoreError> {
    if valid_key(key) {
        Ok(())
    } else {
        Err(CoreError::Validation(vec![FieldError::new(
            field, "invalid",
        )]))
    }
}

fn paths_of(data_dir: &std::path::Path, m: &attachment::Model) -> Vec<PathBuf> {
    [Some(m.path.as_str()), m.thumb_path.as_deref()]
        .into_iter()
        .flatten()
        .filter_map(|rel| store::resolve(data_dir, rel))
        .collect()
}

fn draft_condition(key: &str) -> sea_orm::Condition {
    sea_orm::Condition::all()
        .add(attachment::Column::DraftKey.eq(key))
        .add(attachment::Column::OrderId.is_null())
}

impl Core {
    /// Добавляет фото или файл в черновик приёма.
    pub async fn add_draft_attachment(
        &self,
        actor: &Actor,
        draft_key: &str,
        name: &str,
        bytes: Vec<u8>,
    ) -> Result<Attachment, CoreError> {
        actor.require(Permission::OrdersCreate)?;
        checked_key("draft_key", draft_key)?;
        let data_dir = self.data_dir()?.to_path_buf();
        let too_many = || {
            CoreError::Validation(vec![FieldError::with_message(
                "file",
                "too_many",
                "В черновике уже 10 фото",
            )])
        };
        let count = attachment::Entity::find()
            .filter(draft_condition(draft_key))
            .count(&self.db)
            .await?;
        if count >= MAX_DRAFT_ATTACHMENTS {
            return Err(too_many());
        }
        let written = super::prepare_and_write(
            data_dir,
            format!("attachments/drafts/{draft_key}"),
            name.to_owned(),
            bytes,
        )
        .await?;
        let files = written.files.clone();
        let inserted = async {
            let tx = self.begin_write().await?;
            let count = attachment::Entity::find()
                .filter(draft_condition(draft_key))
                .count(&tx)
                .await?;
            if count >= MAX_DRAFT_ATTACHMENTS {
                return Err(too_many());
            }
            let model = written
                .into_active(None, Some(draft_key.to_owned()), self.now())
                .insert(&tx)
                .await?;
            tx.commit().await?;
            Ok::<_, CoreError>(model)
        }
        .await;
        match inserted {
            Ok(m) => Ok(to_api(m, false)),
            Err(e) => {
                store::remove_files(&files);
                Err(e)
            }
        }
    }

    /// Вложения черновика по порядку добавления.
    pub async fn list_draft_attachments(
        &self,
        actor: &Actor,
        draft_key: &str,
    ) -> Result<Vec<Attachment>, CoreError> {
        actor.require(Permission::OrdersCreate)?;
        checked_key("draft_key", draft_key)?;
        let data_dir = self.data_dir()?;
        let rows = attachment::Entity::find()
            .filter(draft_condition(draft_key))
            .order_by_asc(attachment::Column::Id)
            .all(&self.db)
            .await?;
        Ok(rows
            .into_iter()
            .map(|m| {
                let missing = self.is_missing(data_dir, &m);
                to_api(m, missing)
            })
            .collect())
    }

    /// Выбрасывает черновик: записи и файлы. Неизвестный ключ — не ошибка.
    pub async fn discard_draft(&self, actor: &Actor, draft_key: &str) -> Result<(), CoreError> {
        actor.require(Permission::OrdersCreate)?;
        checked_key("draft_key", draft_key)?;
        let data_dir = self.data_dir()?.to_path_buf();
        let tx = self.begin_write().await?;
        attachment::Entity::delete_many()
            .filter(draft_condition(draft_key))
            .exec(&tx)
            .await?;
        tx.commit().await?;
        remove_draft_dir(&data_dir, draft_key);
        Ok(())
    }

    /// Удаляет черновики старше срока: записи и папки. Возвращает число удалённых вложений.
    pub async fn cleanup_drafts(&self, older_than: Duration) -> Result<u64, CoreError> {
        let Some(data_dir) = self.data_dir.clone() else {
            return Ok(0);
        };
        let cutoff = self.now() - older_than;
        let tx = self.begin_write().await?;
        let expired = attachment::Entity::find()
            .filter(attachment::Column::DraftKey.is_not_null())
            .filter(attachment::Column::OrderId.is_null())
            .filter(attachment::Column::CreatedAt.lt(cutoff))
            .all(&tx)
            .await?;
        if expired.is_empty() {
            tx.rollback().await.ok();
            return Ok(0);
        }
        let ids: Vec<i64> = expired.iter().map(|m| m.id).collect();
        attachment::Entity::delete_many()
            .filter(attachment::Column::Id.is_in(ids))
            .exec(&tx)
            .await?;
        let mut keys: Vec<String> = expired.iter().filter_map(|m| m.draft_key.clone()).collect();
        keys.sort();
        keys.dedup();
        let mut folders = Vec::new();
        for key in keys {
            let left = attachment::Entity::find()
                .filter(draft_condition(&key))
                .count(&tx)
                .await?;
            if left == 0 {
                folders.push(key);
            }
        }
        tx.commit().await?;
        let files: Vec<PathBuf> = expired
            .iter()
            .flat_map(|m| paths_of(&data_dir, m))
            .collect();
        store::remove_files(&files);
        for key in folders {
            remove_draft_dir(&data_dir, &key);
        }
        Ok(expired.len() as u64)
    }

    /// После фиксации заказа: переносит файлы вложений из черновика в папку заказа.
    /// Неудача не отменяет заказ: вложение остаётся по старому пути.
    pub(crate) async fn move_draft_files(&self, order_id: i64, moved: Vec<attachment::Model>) {
        let Some(data_dir) = self.data_dir.clone() else {
            return;
        };
        let draft_key = moved.first().and_then(|m| m.draft_key.clone());
        let updates = tokio::task::spawn_blocking(move || {
            let mut updates: Vec<(i64, String, Option<String>)> = Vec::new();
            for m in moved {
                let Some(file_name) = m.path.rsplit('/').next() else {
                    continue;
                };
                let new_rel = format!("attachments/orders/{order_id}/{file_name}");
                let new_thumb = m.thumb_path.as_ref().map(|_| store::thumb_rel(&new_rel));
                let (Some(from), Some(to)) = (
                    store::resolve(&data_dir, &m.path),
                    store::resolve(&data_dir, &new_rel),
                ) else {
                    continue;
                };
                let moved_ok = (|| -> std::io::Result<()> {
                    if let Some(parent) = to.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::rename(&from, &to)?;
                    if let (Some(old), Some(new)) = (
                        m.thumb_path
                            .as_deref()
                            .and_then(|r| store::resolve(&data_dir, r)),
                        new_thumb
                            .as_deref()
                            .and_then(|r| store::resolve(&data_dir, r)),
                    ) && old.exists()
                    {
                        std::fs::rename(old, new)?;
                    }
                    Ok(())
                })();
                match moved_ok {
                    Ok(()) => updates.push((m.id, new_rel, new_thumb)),
                    Err(e) => tracing::warn!(
                        attachment_id = m.id, error = %e, "файл черновика не перенесён в заказ"
                    ),
                }
            }
            if let Some(key) = &draft_key {
                // Пустая папка черновика не нужна; непустая (неудачный перенос) остаётся.
                std::fs::remove_dir(data_dir.join("attachments/drafts").join(key)).ok();
            }
            updates
        })
        .await
        .unwrap_or_default();
        if updates.is_empty() {
            return;
        }
        let result = async {
            let tx = self.begin_write().await?;
            for (id, path, thumb) in updates {
                attachment::ActiveModel {
                    id: Set(id),
                    path: Set(path),
                    thumb_path: Set(thumb),
                    ..Default::default()
                }
                .update(&tx)
                .await?;
            }
            tx.commit().await?;
            Ok::<(), CoreError>(())
        }
        .await;
        if let Err(e) = result {
            tracing::warn!(order_id, error = %e, "пути вложений после переноса не записаны");
        }
    }
}

fn remove_draft_dir(data_dir: &std::path::Path, key: &str) {
    let dir = data_dir.join("attachments/drafts").join(key);
    match std::fs::remove_dir_all(&dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => tracing::warn!(path = %dir.display(), error = %e, "папка черновика не удалена"),
    }
}
