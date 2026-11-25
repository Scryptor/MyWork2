//! Вложения заказов: фото и файлы. Файлы лежат на диске, в базе — только описание.

mod drafts;
mod logo;
mod photo;
mod store;
pub(crate) mod tree;

use std::path::PathBuf;

use chrono::NaiveDateTime;
use mw_db::entity::attachment;
use mw_domain::permission::Permission;
use mw_domain::validation::FieldError;
use sea_orm::sea_query::Expr;
use sea_orm::{ActiveModelTrait, ColumnTrait, Condition, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};

pub use drafts::MAX_DRAFT_ATTACHMENTS;
pub(crate) use drafts::valid_key;
pub use logo::FirmLogo;
pub use photo::{MAX_ATTACHMENT_BYTES, PhotoError, ProcessedPhoto, process_photo};
pub(crate) use store::remove_files;
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

/// Код вида строки с логотипом фирмы. Только внутри ядра: в `AttachmentKind` его нет, потому что
/// логотип — не вложение заказа и обычным вложениям недоступен.
pub(crate) const LOGO_KIND: &str = "logo";

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
    /// Номер позиции склада (у фото позиции).
    #[schema(required = true)]
    pub stock_item_id: Option<i64>,
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

pub(crate) fn to_api(m: attachment::Model, missing: bool) -> Attachment {
    Attachment {
        id: m.id,
        order_id: m.order_id,
        stock_item_id: m.stock_item_id,
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

/// Право на подпись и удаление вложения: фото позиции склада — у того, кто правит склад; фото
/// приёма (черновика) — у того, кто принимает или правит заказы; вложение заказа — только у того,
/// кто правит заказы.
fn require_edit(actor: &Actor, m: &attachment::Model) -> Result<(), CoreError> {
    if m.stock_item_id.is_some() {
        actor.require(Permission::StockEdit)
    } else if m.order_id.is_none() && m.draft_key.is_some() {
        actor.require_any(&[Permission::OrdersEdit, Permission::OrdersCreate])
    } else {
        actor.require(Permission::OrdersEdit)
    }
}

impl Core {
    pub(crate) fn data_dir(&self) -> Result<&std::path::Path, CoreError> {
        self.data_dir
            .as_deref()
            .ok_or(CoreError::Unavailable("Хранилище файлов не настроено"))
    }

    pub(crate) fn is_missing(&self, data_dir: &std::path::Path, m: &attachment::Model) -> bool {
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
        actor: &Actor,
        id: i64,
        variant: AttachmentVariant,
    ) -> Result<AttachmentFile, CoreError> {
        let data_dir = self.data_dir()?;
        let not_found = || CoreError::NotFound { entity: ENTITY, id };
        let m = attachment::Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .filter(|m| m.kind != LOGO_KIND)
            .ok_or_else(not_found)?;
        // Фото позиции склада видит тот, кто видит склад; остальные вложения — любой вошедший.
        if m.stock_item_id.is_some() {
            actor.require(Permission::StockView)?;
        }
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

    /// Меняет подпись; без изменений ничего не пишет. Права — как у `require_edit`.
    pub async fn update_attachment_caption(
        &self,
        actor: &Actor,
        id: i64,
        caption: String,
    ) -> Result<Attachment, CoreError> {
        let data_dir = self.data_dir()?;
        let caption = caption.trim().to_owned();
        let tx = self.begin_write().await?;
        let current = attachment::Entity::find_by_id(id)
            .one(&tx)
            .await?
            .filter(|m| m.kind != LOGO_KIND)
            .ok_or(CoreError::NotFound { entity: ENTITY, id })?;
        require_edit(actor, &current)?;
        if caption.chars().count() > MAX_CAPTION_CHARS {
            return Err(CoreError::Validation(vec![FieldError::new(
                "caption", "too_long",
            )]));
        }
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
                stock_item_id: current.stock_item_id,
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

    /// Удаляет вложение; файлы убираются с диска после фиксации записи. Права — как у
    /// `require_edit`.
    pub async fn delete_attachment(&self, actor: &Actor, id: i64) -> Result<(), CoreError> {
        let data_dir = self.data_dir()?.to_path_buf();
        let tx = self.begin_write().await?;
        let current = attachment::Entity::find_by_id(id)
            .one(&tx)
            .await?
            .filter(|m| m.kind != LOGO_KIND)
            .ok_or(CoreError::NotFound { entity: ENTITY, id })?;
        require_edit(actor, &current)?;
        attachment::Entity::delete_by_id(id).exec(&tx).await?;
        write_audit(
            &tx,
            self.now(),
            actor,
            NewAuditEntry {
                action: "attachment.deleted",
                order_id: current.order_id,
                client_id: None,
                stock_item_id: current.stock_item_id,
                summary: format!("Удалён файл «{}»", current.name),
                details: Some(serde_json::json!({
                    "name": current.name,
                    "path": current.path,
                })),
            },
        )
        .await?;
        tx.commit().await?;
        // Для вложения черновика уходит и опустевшая папка черновика.
        drafts::remove_rows_files(&data_dir, std::slice::from_ref(&current));
        Ok(())
    }
}

/// Файлы, записанные на диск до записи в базу.
pub(crate) struct Written {
    pub prepared: photo::Prepared,
    pub path: String,
    pub thumb_path: Option<String>,
    pub size: i64,
    pub files: Vec<PathBuf>,
}

/// Разбирает байты и пишет файлы в `rel_dir` (путь внутри папки данных). Блокирующая работа.
pub(crate) async fn prepare_and_write(
    data_dir: PathBuf,
    rel_dir: String,
    name: String,
    bytes: Vec<u8>,
) -> Result<Written, CoreError> {
    tokio::task::spawn_blocking(move || {
        let size = i64::try_from(bytes.len()).unwrap_or(i64::MAX);
        let prepared = photo::prepare_upload(&name, bytes)?;
        let file_name = if prepared.ext.is_empty() {
            uuid::Uuid::new_v4().to_string()
        } else {
            format!("{}.{}", uuid::Uuid::new_v4(), prepared.ext)
        };
        let path = format!("{rel_dir}/{file_name}");
        let main = store::resolve(&data_dir, &path)
            .ok_or_else(|| CoreError::Internal("недопустимый путь вложения".into()))?;
        let mut files = vec![main.clone()];
        let thumb_path = prepared.thumb.as_ref().map(|_| store::thumb_rel(&path));
        let mut write = || -> std::io::Result<()> {
            if let Some(parent) = main.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&main, &prepared.main)?;
            if let (Some(rel), Some(thumb)) = (&thumb_path, &prepared.thumb)
                && let Some(p) = store::resolve(&data_dir, rel)
            {
                files.push(p.clone());
                std::fs::write(p, thumb)?;
            }
            Ok(())
        };
        if let Err(e) = write() {
            store::remove_files(&files);
            return Err(CoreError::Internal(format!("файл не записан: {e}")));
        }
        let size = if prepared.photo {
            i64::try_from(prepared.main.len()).unwrap_or(size)
        } else {
            size
        };
        Ok(Written {
            prepared,
            path,
            thumb_path,
            size,
            files,
        })
    })
    .await
    .map_err(|e| CoreError::Internal(e.to_string()))?
}

impl Written {
    pub(crate) fn into_active(
        self,
        order_id: Option<i64>,
        draft_key: Option<String>,
        created_at: NaiveDateTime,
    ) -> attachment::ActiveModel {
        let p = self.prepared;
        attachment::ActiveModel {
            id: sea_orm::NotSet,
            order_id: Set(order_id),
            draft_key: Set(draft_key),
            kind: Set(if p.photo {
                AttachmentKind::Photo.code()
            } else {
                AttachmentKind::File.code()
            }
            .to_owned()),
            name: Set(p.name),
            caption: Set(String::new()),
            content_type: Set(p.content_type),
            size: Set(self.size),
            width: Set(p.width),
            height: Set(p.height),
            path: Set(self.path),
            thumb_path: Set(self.thumb_path),
            legacy: Set(false),
            created_at: Set(created_at),
            stock_item_id: Set(None),
        }
    }

    /// Фото позиции склада: без заказа и черновика.
    pub(crate) fn into_stock_active(
        self,
        stock_item_id: i64,
        created_at: NaiveDateTime,
    ) -> attachment::ActiveModel {
        attachment::ActiveModel {
            stock_item_id: Set(Some(stock_item_id)),
            ..self.into_active(None, None, created_at)
        }
    }
}

impl Core {
    /// Добавляет фото или файл к заказу.
    pub async fn add_order_attachment(
        &self,
        actor: &Actor,
        order_id: i64,
        name: &str,
        bytes: Vec<u8>,
    ) -> Result<Attachment, CoreError> {
        actor.require(Permission::OrdersEdit)?;
        let data_dir = self.data_dir()?.to_path_buf();
        if mw_db::entity::order::Entity::find_by_id(order_id)
            .one(&self.db)
            .await?
            .is_none()
        {
            return Err(CoreError::NotFound {
                entity: "заказ",
                id: order_id,
            });
        }
        let written = prepare_and_write(
            data_dir.clone(),
            format!("attachments/orders/{order_id}"),
            name.to_owned(),
            bytes,
        )
        .await?;
        let files = written.files.clone();
        let inserted = async {
            let tx = self.begin_write().await?;
            let now = self.now();
            let model = written
                .into_active(Some(order_id), None, now)
                .insert(&tx)
                .await?;
            write_audit(
                &tx,
                now,
                actor,
                NewAuditEntry {
                    action: "attachment.added",
                    order_id: Some(order_id),
                    client_id: None,
                    stock_item_id: None,
                    summary: format!("Добавлен файл «{}»", model.name),
                    details: Some(serde_json::json!({
                        "name": model.name,
                        "kind": model.kind,
                        "size": model.size,
                    })),
                },
            )
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
}
