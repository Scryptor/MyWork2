//! Лента комментариев заказа.

use chrono::NaiveDateTime;
use mw_db::entity::order_comment;
use mw_domain::{permission::Permission, validation::FieldError};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde::{Deserialize, Serialize};

use super::update::{deleted_error, load};
use crate::{Actor, Core, CoreError, NewAuditEntry, Page, PageRequest, write_audit};
use crate::{attachments::valid_key, page::validate_page};

/// Текст комментария и ключ повтора одной отправки.
#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewOrderComment {
    pub body: String,
    /// От 1 до 64 латинских букв, цифр и дефисов.
    pub request_key: String,
}

/// Запись ленты с именем автора на момент добавления и местным временем предприятия.
#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct OrderComment {
    pub id: i64,
    pub body: String,
    pub author_name: String,
    pub created_at: NaiveDateTime,
}

impl From<order_comment::Model> for OrderComment {
    fn from(row: order_comment::Model) -> Self {
        Self {
            id: row.id,
            body: row.body,
            author_name: row.author_name,
            created_at: row.created_at,
        }
    }
}

impl Core {
    /// По порядку добавления, новые сверху. Как в истории, удалённый заказ доступен для чтения.
    pub async fn order_comments(
        &self,
        _actor: &Actor,
        id: i64,
        page: PageRequest,
        before_id: Option<i64>,
    ) -> Result<Page<OrderComment>, CoreError> {
        validate_page(page.limit, page.offset)?;
        load(&self.db, id).await?;
        let query = order_comment::Entity::find().filter(order_comment::Column::OrderId.eq(id));
        let total = query.clone().count(&self.db).await?;
        let query = match before_id {
            Some(cursor) => query.filter(order_comment::Column::Id.lt(cursor)),
            None => query,
        };
        let items = query
            .order_by_desc(order_comment::Column::Id)
            .limit(page.limit)
            .offset(page.offset)
            .all(&self.db)
            .await?
            .into_iter()
            .map(OrderComment::from)
            .collect();
        Ok(Page { items, total })
    }

    /// Повторная отправка тем же автором с тем же ключом и текстом возвращает прежнюю запись.
    pub async fn add_order_comment(
        &self,
        actor: &Actor,
        id: i64,
        new: NewOrderComment,
    ) -> Result<OrderComment, CoreError> {
        actor.require(Permission::OrdersEdit)?;
        let body = new.body.trim();
        if body.is_empty() {
            return Err(CoreError::Validation(vec![FieldError::required("body")]));
        }
        if !valid_key(&new.request_key) {
            return Err(CoreError::Validation(vec![FieldError::new(
                "request_key",
                "invalid",
            )]));
        }
        let tx = self.begin_write().await?;
        let order = load(&tx, id).await?;
        if order.deleted_at.is_some() {
            return Err(deleted_error());
        }
        if let Some(row) = order_comment::Entity::find()
            .filter(order_comment::Column::OrderId.eq(id))
            .filter(order_comment::Column::RequestKey.eq(&new.request_key))
            .one(&tx)
            .await?
        {
            if row.body != body || row.author_id != actor.user_id() {
                return Err(CoreError::Validation(vec![FieldError::new(
                    "request_key",
                    "duplicate",
                )]));
            }
            tx.commit().await?;
            return Ok(row.into());
        }
        let at = self.now();
        let row = order_comment::ActiveModel {
            order_id: Set(id),
            request_key: Set(new.request_key),
            body: Set(body.to_owned()),
            author_name: Set(actor.name().to_owned()),
            author_id: Set(actor.user_id()),
            created_at: Set(at),
            ..Default::default()
        }
        .insert(&tx)
        .await?;
        write_audit(
            &tx,
            at,
            actor,
            NewAuditEntry {
                action: "order.comment",
                order_id: Some(id),
                client_id: Some(order.client_id),
                stock_item_id: None,
                summary: "Добавлен комментарий к заказу".to_owned(),
                details: Some(serde_json::json!({"comment_id":row.id})),
            },
        )
        .await?;
        tx.commit().await?;
        Ok(row.into())
    }
}
