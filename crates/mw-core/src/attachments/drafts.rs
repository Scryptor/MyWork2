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
    /// Неудача не отменяет заказ: вложение остаётся по старому пути, а частично сделанный
    /// перенос откатывается, чтобы путь в базе и файл на диске не расходились.
    pub(crate) async fn move_draft_files(&self, order_id: i64, rows: Vec<attachment::Model>) {
        let Some(data_dir) = self.data_dir.clone() else {
            return;
        };
        let draft_key = rows.first().and_then(|m| m.draft_key.clone());
        let moved = {
            let data_dir = data_dir.clone();
            tokio::task::spawn_blocking(move || move_files(&data_dir, order_id, rows))
                .await
                .unwrap_or_default()
        };
        if let Some(key) = &draft_key {
            // Пустая папка черновика не нужна; непустая (неудачный перенос) остаётся.
            std::fs::remove_dir(data_dir.join("attachments/drafts").join(key)).ok();
        }
        if moved.is_empty() {
            return;
        }
        let result = async {
            let tx = self.begin_write().await?;
            for m in &moved {
                attachment::ActiveModel {
                    id: Set(m.id),
                    path: Set(m.new_path.clone()),
                    thumb_path: Set(m.new_thumb.clone()),
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
            for m in moved.iter().rev() {
                undo(&m.done);
            }
        }
    }
}

/// Перенесённое вложение: новые пути для базы и сделанные переименования для отката.
#[derive(Debug)]
pub(crate) struct MovedAttachment {
    pub id: i64,
    pub new_path: String,
    pub new_thumb: Option<String>,
    pub done: Vec<(PathBuf, PathBuf)>,
}

/// Возвращает файлы на прежние места (в обратном порядке).
fn undo(done: &[(PathBuf, PathBuf)]) {
    for (from, to) in done.iter().rev() {
        if let Err(e) = std::fs::rename(to, from) {
            tracing::warn!(path = %from.display(), error = %e, "перенос вложения не откатился");
        }
    }
}

/// Переносит файлы вложений в папку заказа. Вложение либо переносится целиком (основной
/// файл и миниатюра, если она есть на диске), либо остаётся на месте.
pub(crate) fn move_files(
    data_dir: &std::path::Path,
    order_id: i64,
    rows: Vec<attachment::Model>,
) -> Vec<MovedAttachment> {
    let mut out = Vec::new();
    for m in rows {
        let Some(file_name) = m.path.rsplit('/').next() else {
            continue;
        };
        let new_rel = format!("attachments/orders/{order_id}/{file_name}");
        let (Some(from), Some(to)) = (
            store::resolve(data_dir, &m.path),
            store::resolve(data_dir, &new_rel),
        ) else {
            continue;
        };
        let mut pairs = vec![(from, to)];
        let mut new_thumb = None;
        if let Some(old) = m
            .thumb_path
            .as_deref()
            .and_then(|r| store::resolve(data_dir, r))
            && old.exists()
        {
            let rel = store::thumb_rel(&new_rel);
            if let Some(new) = store::resolve(data_dir, &rel) {
                pairs.push((old, new));
                new_thumb = Some(rel);
            }
        }
        let mut done: Vec<(PathBuf, PathBuf)> = Vec::new();
        let mut failure = None;
        for (from, to) in pairs {
            let step = (|| -> std::io::Result<()> {
                if let Some(parent) = to.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::rename(&from, &to)
            })();
            match step {
                Ok(()) => done.push((from, to)),
                Err(e) => {
                    failure = Some(e);
                    break;
                }
            }
        }
        if let Some(e) = failure {
            undo(&done);
            tracing::warn!(
                attachment_id = m.id, error = %e, "файл черновика не перенесён в заказ"
            );
            continue;
        }
        out.push(MovedAttachment {
            id: m.id,
            new_path: new_rel,
            new_thumb,
            done,
        });
    }
    out
}

fn remove_draft_dir(data_dir: &std::path::Path, key: &str) {
    let dir = data_dir.join("attachments/drafts").join(key);
    match std::fs::remove_dir_all(&dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => tracing::warn!(path = %dir.display(), error = %e, "папка черновика не удалена"),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn row(id: i64, path: &str, thumb: Option<&str>) -> attachment::Model {
        attachment::Model {
            id,
            order_id: None,
            draft_key: Some("k".into()),
            kind: "photo".into(),
            name: "a.jpg".into(),
            caption: String::new(),
            content_type: "image/jpeg".into(),
            size: 1,
            width: None,
            height: None,
            path: path.into(),
            thumb_path: thumb.map(str::to_owned),
            legacy: false,
            created_at: chrono::NaiveDateTime::default(),
        }
    }

    fn put(dir: &std::path::Path, rel: &str) {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"x").unwrap();
    }

    #[test]
    fn failed_thumbnail_move_puts_the_main_file_back() {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), "attachments/drafts/k/a.jpg");
        put(dir.path(), "attachments/drafts/k/a.jpg.thumb.jpg");
        // Целевое имя миниатюры занято непустой папкой: переименование не удастся.
        put(dir.path(), "attachments/orders/7/a.jpg.thumb.jpg/inner");
        let rows = vec![row(
            1,
            "attachments/drafts/k/a.jpg",
            Some("attachments/drafts/k/a.jpg.thumb.jpg"),
        )];
        assert!(move_files(dir.path(), 7, rows).is_empty());
        assert!(dir.path().join("attachments/drafts/k/a.jpg").exists());
        assert!(
            dir.path()
                .join("attachments/drafts/k/a.jpg.thumb.jpg")
                .exists()
        );
        assert!(!dir.path().join("attachments/orders/7/a.jpg").exists());
    }

    #[test]
    fn missing_thumbnail_is_not_recorded_and_undo_restores_files() {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), "attachments/drafts/k/a.jpg");
        put(dir.path(), "attachments/drafts/k/b.jpg");
        put(dir.path(), "attachments/drafts/k/b.jpg.thumb.jpg");
        let rows = vec![
            row(
                1,
                "attachments/drafts/k/a.jpg",
                Some("attachments/drafts/k/a.jpg.thumb.jpg"),
            ),
            row(
                2,
                "attachments/drafts/k/b.jpg",
                Some("attachments/drafts/k/b.jpg.thumb.jpg"),
            ),
        ];
        let moved = move_files(dir.path(), 7, rows);
        assert_eq!(moved.len(), 2);
        assert_eq!(moved[0].new_thumb, None);
        assert_eq!(
            moved[1].new_thumb.as_deref(),
            Some("attachments/orders/7/b.jpg.thumb.jpg")
        );
        for m in moved.iter().rev() {
            undo(&m.done);
        }
        for rel in [
            "attachments/drafts/k/a.jpg",
            "attachments/drafts/k/b.jpg",
            "attachments/drafts/k/b.jpg.thumb.jpg",
        ] {
            assert!(dir.path().join(rel).exists(), "{rel}");
        }
    }
}
