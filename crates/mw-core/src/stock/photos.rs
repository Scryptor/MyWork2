//! Фото позиций склада: до `MAX_STOCK_PHOTOS` штук, права `stock_view` / `stock_edit`.

use mw_db::entity::{attachment, stock_item};
use mw_domain::permission::Permission;
use mw_domain::stock::MAX_STOCK_PHOTOS;
use mw_domain::validation::FieldError;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder,
};

use super::{ENTITY, deleted_error};
use crate::attachments::{Attachment, prepare_and_write, remove_files, to_api};
use crate::{Actor, Core, CoreError, NewAuditEntry, write_audit};

fn too_many() -> CoreError {
    CoreError::Validation(vec![FieldError::with_message(
        "file",
        "too_many",
        "У позиции уже 3 фото",
    )])
}

fn not_a_photo() -> CoreError {
    CoreError::Validation(vec![FieldError::with_message(
        "file",
        "not_a_photo",
        "У позиции склада — только фото",
    )])
}

fn max_photos() -> u64 {
    u64::try_from(MAX_STOCK_PHOTOS).unwrap_or(u64::MAX)
}

fn photos_of(id: i64) -> sea_orm::Select<attachment::Entity> {
    attachment::Entity::find().filter(attachment::Column::StockItemId.eq(id))
}

/// Позиция есть и не удалена: иначе «не найдена» или `id` `deleted`.
async fn require_live(
    conn: &impl ConnectionTrait,
    id: i64,
) -> Result<stock_item::Model, CoreError> {
    let m = stock_item::Entity::find_by_id(id)
        .one(conn)
        .await?
        .ok_or(CoreError::NotFound { entity: ENTITY, id })?;
    if m.deleted_at.is_some() {
        return Err(deleted_error());
    }
    Ok(m)
}

impl Core {
    /// Фото позиции по времени добавления (право `stock_view`); у удалённой позиции они остаются
    /// видны, как и сама история.
    pub async fn list_stock_photos(
        &self,
        actor: &Actor,
        id: i64,
    ) -> Result<Vec<Attachment>, CoreError> {
        actor.require(Permission::StockView)?;
        let data_dir = self.data_dir()?;
        let rows = photos_of(id)
            .order_by_asc(attachment::Column::CreatedAt)
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

    /// Добавляет фото позиции (право `stock_edit`): не больше `MAX_STOCK_PHOTOS`, только
    /// картинки. Предел проверяется ещё раз внутри транзакции записи — одновременные загрузки
    /// не дадут четвёртого фото.
    pub async fn add_stock_photo(
        &self,
        actor: &Actor,
        id: i64,
        name: &str,
        bytes: Vec<u8>,
    ) -> Result<Attachment, CoreError> {
        actor.require(Permission::StockEdit)?;
        let data_dir = self.data_dir()?.to_path_buf();
        require_live(&self.db, id).await?;
        if photos_of(id).count(&self.db).await? >= max_photos() {
            return Err(too_many());
        }
        let written = prepare_and_write(
            data_dir,
            format!("attachments/stock/{id}"),
            name.to_owned(),
            bytes,
        )
        .await?;
        let files = written.files.clone();
        let inserted = async {
            if !written.prepared.photo {
                return Err(not_a_photo());
            }
            let tx = self.begin_write().await?;
            require_live(&tx, id).await?;
            if photos_of(id).count(&tx).await? >= max_photos() {
                return Err(too_many());
            }
            let now = self.now();
            let model = written.into_stock_active(id, now).insert(&tx).await?;
            write_audit(
                &tx,
                now,
                actor,
                NewAuditEntry {
                    action: "attachment.added",
                    order_id: None,
                    client_id: None,
                    stock_item_id: Some(id),
                    summary: format!("Добавлено фото «{}»", model.name),
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
                remove_files(&files);
                Err(e)
            }
        }
    }
}
