//! Вложения заказов: фото и файлы. Файлы лежат на диске, в базе — только описание.

mod store;

use std::path::PathBuf;

use chrono::NaiveDateTime;
use mw_db::entity::attachment;
use mw_domain::permission::Permission;
use mw_domain::validation::FieldError;
use sea_orm::sea_query::Expr;
use sea_orm::{ActiveModelTrait, ColumnTrait, Condition, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};

pub use store::resolve;

use crate::{Actor, Core, CoreError, NewAuditEntry, write_audit};

const ENTITY: &str = "вложение";
const MAX_CAPTION_CHARS: usize = 500;

/// Вид вложения.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentKind {
    Photo,
    File,
}

impl AttachmentKind {
    fn code(self) -> &'static str {
        match self {
            Self::Photo => "photo",
            Self::File => "file",
        }
    }
}

/// Какой файл отдать.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentVariant {
    Full,
    Thumb,
}

/// Вложение заказа.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct Attachment {
    /// Номер вложения.
    pub id: i64,
    /// Номер заказа.
    #[schema(required = true)]
    pub order_id: Option<i64>,
    pub kind: AttachmentKind,
    /// Имя файла для человека.
    pub name: String,
    /// Подпись.
    pub caption: String,
    pub content_type: String,
    /// Размер в байтах.
    pub size: i64,
    #[schema(required = true)]
    pub width: Option<i32>,
    #[schema(required = true)]
    pub height: Option<i32>,
    /// Файл из старой программы.
    pub legacy: bool,
    /// Файла нет на диске.
    pub missing: bool,
    pub created_at: NaiveDateTime,
}

/// Файл вложения для отдачи.
#[derive(Debug, Clone, PartialEq)]
pub struct AttachmentFile {
    pub path: PathBuf,
    pub content_type: String,
    pub name: String,
}

fn kind_of(m: &attachment::Model) -> AttachmentKind {
    if m.kind == AttachmentKind::Photo.code() {
        AttachmentKind::Photo
    } else {
        AttachmentKind::File
    }
}

fn to_api(m: attachment::Model, missing: bool) -> Attachment {
    Attachment {
        id: m.id,
        order_id: m.order_id,
        kind: kind_of(&m),
        name: m.name,
        caption: m.caption,
        content_type: m.content_type,
        size: m.size,
        width: m.width,
        height: m.height,
        legacy: m.legacy,
        missing,
        created_at: m.created_at,
    }
}

impl Core {
    fn data_dir(&self) -> Result<&std::path::Path, CoreError> {
        self.data_dir
            .as_deref()
            .ok_or(CoreError::Unavailable("Хранилище файлов не настроено"))
    }

    fn is_missing(&self, data_dir: &std::path::Path, m: &attachment::Model) -> bool {
        store::resolve(data_dir, &m.path).is_none_or(|p| !p.exists())
    }

    /// Вложения заказа: сначала фото, затем файлы; внутри — по времени добавления.
    pub async fn list_order_attachments(
        &self,
        _actor: &Actor,
        order_id: i64,
    ) -> Result<Vec<Attachment>, CoreError> {
        let data_dir = self.data_dir()?;
        let mut rows = attachment::Entity::find()
            .filter(attachment::Column::OrderId.eq(order_id))
            .all(&self.db)
            .await?;
        rows.sort_by_key(|m| (kind_of(m) == AttachmentKind::File, m.created_at, m.id));
        Ok(rows
            .into_iter()
            .map(|m| {
                let missing = self.is_missing(data_dir, &m);
                to_api(m, missing)
            })
            .collect())
    }

    /// Путь к файлу вложения или его миниатюры. У старых фото миниатюра строится при первом запросе.
    pub async fn attachment_file(
        &self,
        _actor: &Actor,
        id: i64,
        variant: AttachmentVariant,
    ) -> Result<AttachmentFile, CoreError> {
        let data_dir = self.data_dir()?;
        let not_found = || CoreError::NotFound { entity: ENTITY, id };
        let m = attachment::Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(not_found)?;
        let main = store::resolve(data_dir, &m.path)
            .filter(|p| p.exists())
            .ok_or_else(not_found)?;
        if variant == AttachmentVariant::Full {
            return Ok(AttachmentFile {
                path: main,
                content_type: m.content_type,
                name: m.name,
            });
        }
        if kind_of(&m) != AttachmentKind::Photo {
            return Err(not_found());
        }
        let stem = m.name.rsplit_once('.').map_or(m.name.as_str(), |(s, _)| s);
        let thumb_name = format!("{stem}.thumb.jpg");
        let existing = m
            .thumb_path
            .as_deref()
            .and_then(|rel| store::resolve(data_dir, rel))
            .filter(|p| p.exists());
        if let Some(path) = existing {
            return Ok(AttachmentFile {
                path,
                content_type: "image/jpeg".into(),
                name: thumb_name,
            });
        }
        let rel = store::thumb_rel(&m.path);
        let target = store::resolve(data_dir, &rel).ok_or_else(not_found)?;
        let built = {
            let (from, to) = (main.clone(), target.clone());
            tokio::task::spawn_blocking(move || store::write_thumbnail(&from, &to))
                .await
                .map_err(|e| CoreError::Internal(e.to_string()))?
        };
        if let Err(e) = built {
            std::fs::remove_file(&target).ok();
            tracing::warn!(attachment_id = id, error = %e, "миниатюра не построена");
            return Err(not_found());
        }
        // Условная запись: если миниатюру уже записал другой запрос, ничего не меняется.
        let tx = self.begin_write().await?;
        attachment::Entity::update_many()
            .col_expr(attachment::Column::ThumbPath, Expr::value(rel.clone()))
            .filter(attachment::Column::Id.eq(id))
            .filter(
                Condition::any()
                    .add(attachment::Column::ThumbPath.is_null())
                    .add(attachment::Column::ThumbPath.ne(rel.clone())),
            )
            .exec(&tx)
            .await?;
        let still_there = attachment::Entity::find_by_id(id).one(&tx).await?.is_some();
        tx.commit().await?;
        if !still_there {
            // Вложение удалили во время построения: файл больше не нужен.
            store::remove_files(std::slice::from_ref(&target));
            return Err(not_found());
        }
        Ok(AttachmentFile {
            path: target,
            content_type: "image/jpeg".into(),
            name: thumb_name,
        })
    }

    /// Меняет подпись; без изменений ничего не пишет.
    pub async fn update_attachment_caption(
        &self,
        actor: &Actor,
        id: i64,
        caption: String,
    ) -> Result<Attachment, CoreError> {
        actor.require(Permission::OrdersEdit)?;
        let data_dir = self.data_dir()?;
        let caption = caption.trim().to_owned();
        if caption.chars().count() > MAX_CAPTION_CHARS {
            return Err(CoreError::Validation(vec![FieldError::new(
                "caption", "too_long",
            )]));
        }
        let tx = self.begin_write().await?;
        let current = attachment::Entity::find_by_id(id)
            .one(&tx)
            .await?
            .ok_or(CoreError::NotFound { entity: ENTITY, id })?;
        if current.caption == caption {
            tx.rollback().await.ok();
            let missing = self.is_missing(data_dir, &current);
            return Ok(to_api(current, missing));
        }
        let updated = attachment::ActiveModel {
            id: Set(id),
            caption: Set(caption.clone()),
            ..Default::default()
        }
        .update(&tx)
        .await?;
        write_audit(
            &tx,
            self.now(),
            actor,
            NewAuditEntry {
                action: "attachment.caption",
                order_id: current.order_id,
                client_id: None,
                summary: format!("Подпись к файлу «{}»: {caption}", current.name),
                details: Some(serde_json::json!({
                    "name": current.name,
                    "from": current.caption,
                    "to": caption,
                })),
            },
        )
        .await?;
        tx.commit().await?;
        let missing = self.is_missing(data_dir, &updated);
        Ok(to_api(updated, missing))
    }

    /// Удаляет вложение; файлы убираются с диска после фиксации записи.
    pub async fn delete_attachment(&self, actor: &Actor, id: i64) -> Result<(), CoreError> {
        actor.require(Permission::OrdersEdit)?;
        let data_dir = self.data_dir()?.to_path_buf();
        let tx = self.begin_write().await?;
        let current = attachment::Entity::find_by_id(id)
            .one(&tx)
            .await?
            .ok_or(CoreError::NotFound { entity: ENTITY, id })?;
        attachment::Entity::delete_by_id(id).exec(&tx).await?;
        write_audit(
            &tx,
            self.now(),
            actor,
            NewAuditEntry {
                action: "attachment.deleted",
                order_id: current.order_id,
                client_id: None,
                summary: format!("Удалён файл «{}»", current.name),
                details: Some(serde_json::json!({
                    "name": current.name,
                    "path": current.path,
                })),
            },
        )
        .await?;
        tx.commit().await?;
        let files: Vec<PathBuf> = [Some(current.path.as_str()), current.thumb_path.as_deref()]
            .into_iter()
            .flatten()
            .filter_map(|rel| store::resolve(&data_dir, rel))
            .collect();
        store::remove_files(&files);
        Ok(())
    }
}
