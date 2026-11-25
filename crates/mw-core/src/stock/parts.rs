//! Запчасти заказа: списание со склада в заказ и возврат на склад.
//!
//! Списание и возврат идут одной транзакцией `begin_write()`: писатели в обеих СУБД выстроены по
//! одному, поэтому проверка ключа повтора и вставка строки не разделяются чужой записью, а
//! уникальный индекс `(order_id, request_key)` остаётся только страховкой (как у комментариев).
//! Остаток меняет условный `UPDATE`: он не уводит остаток ниже нуля, даже если проверка выше
//! чем-то обойдена. Версия заказа не меняется, версия позиции — меняется.

use std::collections::HashSet;

use chrono::NaiveDateTime;
use mw_db::entity::{order, order_part, stock_item};
use mw_domain::money::Money;
use mw_domain::permission::Permission;
use mw_domain::stock::{MAX_STOCK_QUANTITY, part_sum};
use mw_domain::validation::FieldError;
use sea_orm::sea_query::{Expr, ExprTrait};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, NotSet, QueryFilter, QueryOrder,
    Set,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::places::{load_places, reduce_places, reduced_json, reduced_tail};
use super::{ENTITY, deleted_error, display_name};
use crate::attachments::valid_key;
use crate::money_text::MoneyText;
use crate::{Actor, Core, CoreError, NewAuditEntry, write_audit};

/// Название заказа в ошибке «не найдено» (как в `orders`).
const ORDER_ENTITY: &str = "заказ";
/// Название строки запчасти в ошибке «не найдено».
const PART_ENTITY: &str = "запчасть заказа";

/// Запчасть заказа: снимок наименования и цены позиции на момент списания.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct OrderPart {
    /// Номер строки.
    pub id: i64,
    /// Позиция склада; пусто — позиции не было уже в MyWork 2.
    #[schema(required = true)]
    pub stock_item_id: Option<i64>,
    /// Позиция есть на складе (не удалена).
    pub item_exists: bool,
    /// Наименование позиции на момент списания.
    pub name: String,
    /// Цена за штуку на момент списания, в копейках.
    pub price: Money,
    /// Сколько штук списано.
    pub quantity: i64,
    /// Цена × количество, в копейках.
    pub sum: Money,
    /// Кто списал (имя на момент списания); у перенесённых из MyWork 2 — пусто.
    pub added_by_name: String,
    /// Когда списана (местное время предприятия).
    pub added_at: NaiveDateTime,
    /// Перенесена из MyWork 2.
    pub legacy: bool,
    /// Коды ячеек, где сейчас лежит живая позиция; пусто без права `stock_view` или без позиции.
    pub places: Vec<String>,
}

/// Запчасти заказа по времени списания и сумма всех строк.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct OrderParts {
    /// Строки по времени списания, затем по номеру.
    pub parts: Vec<OrderPart>,
    /// Σ цена × количество, в копейках.
    pub total: Money,
}

/// Списание позиции склада в заказ.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewOrderPart {
    /// Номер позиции склада.
    pub stock_item_id: i64,
    /// Сколько штук списать: 1…1 000 000.
    pub quantity: i64,
    /// Ключ повтора запроса: от 1 до 64 латинских букв, цифр и дефисов. Повтор с тем же ключом и
    /// тем же содержимым возвращает запчасти заказа без нового списания.
    pub request_key: String,
}

/// Списание и возврат: правка заказа и доступ к складу.
fn require_write_off(actor: &Actor) -> Result<(), CoreError> {
    actor.require(Permission::OrdersEdit)?;
    actor.require(Permission::StockView)
}

fn validation(field: &str, code: &str) -> CoreError {
    CoreError::Validation(vec![FieldError::new(field, code)])
}

/// Сумма не помещается в копейки `i64`: при пределах позиции склада недостижимо.
fn overflow() -> CoreError {
    CoreError::Internal("сумма запчастей заказа не помещается в число".into())
}

async fn load_order(conn: &impl ConnectionTrait, id: i64) -> Result<order::Model, CoreError> {
    order::Entity::find_by_id(id)
        .one(conn)
        .await?
        .ok_or(CoreError::NotFound {
            entity: ORDER_ENTITY,
            id,
        })
}

/// Заказ есть и не удалён: у удалённого запчасти только читаются.
async fn live_order(conn: &impl ConnectionTrait, id: i64) -> Result<order::Model, CoreError> {
    let order = load_order(conn, id).await?;
    if order.deleted_at.is_some() {
        return Err(deleted_error());
    }
    Ok(order)
}

/// Запчасти заказа по времени списания и номеру, с признаком «позиция есть на складе» и суммой.
async fn order_parts(
    conn: &impl ConnectionTrait,
    order_id: i64,
    show_places: bool,
) -> Result<OrderParts, CoreError> {
    let rows = order_part::Entity::find()
        .filter(order_part::Column::OrderId.eq(order_id))
        .order_by_asc(order_part::Column::AddedAt)
        .order_by_asc(order_part::Column::Id)
        .all(conn)
        .await?;
    let item_ids: HashSet<i64> = rows.iter().filter_map(|r| r.stock_item_id).collect();
    let live: HashSet<i64> = if item_ids.is_empty() {
        HashSet::new()
    } else {
        stock_item::Entity::find()
            .filter(stock_item::Column::Id.is_in(item_ids))
            .filter(stock_item::Column::DeletedAt.is_null())
            .all(conn)
            .await?
            .into_iter()
            .map(|m| m.id)
            .collect()
    };
    let live_ids: Vec<i64> = live.iter().copied().collect();
    let places = if show_places {
        load_places(conn, &live_ids).await?
    } else {
        Default::default()
    };
    let mut total: i64 = 0;
    let mut parts = Vec::with_capacity(rows.len());
    for r in rows {
        let price = Money::from_kopecks(r.price);
        let sum = part_sum(price, r.quantity).ok_or_else(overflow)?;
        total = total.checked_add(sum.kopecks()).ok_or_else(overflow)?;
        parts.push(OrderPart {
            id: r.id,
            item_exists: r.stock_item_id.is_some_and(|id| live.contains(&id)),
            stock_item_id: r.stock_item_id,
            name: r.name,
            price,
            quantity: r.quantity,
            sum,
            added_by_name: r.added_by_name,
            added_at: r.added_at,
            legacy: r.legacy,
            places: r
                .stock_item_id
                .and_then(|id| places.get(&id))
                .map(|list| list.iter().map(|p| p.code.clone()).collect())
                .unwrap_or_default(),
        });
    }
    Ok(OrderParts {
        parts,
        total: Money::from_kopecks(total),
    })
}

impl Core {
    /// Запчасти заказа. Права — как у самого заказа (достаточно войти); удалённый заказ читается.
    pub async fn list_order_parts(
        &self,
        actor: &Actor,
        order_id: i64,
    ) -> Result<OrderParts, CoreError> {
        load_order(&self.db, order_id).await?;
        let show_places = actor.require(Permission::StockView).is_ok();
        order_parts(&self.db, order_id, show_places).await
    }

    /// Списание позиции склада в заказ (права `orders_edit` и `stock_view`): остаток позиции
    /// уменьшается, её версия растёт, в заказе — строка со снимком наименования и цены. Версия
    /// заказа не меняется. Возвращает запчасти заказа.
    pub async fn add_order_part(
        &self,
        actor: &Actor,
        order_id: i64,
        new: NewOrderPart,
    ) -> Result<OrderParts, CoreError> {
        require_write_off(actor)?;
        let mut errors = Vec::new();
        if !(1..=MAX_STOCK_QUANTITY).contains(&new.quantity) {
            errors.push(FieldError::new("quantity", "out_of_range"));
        }
        if !valid_key(&new.request_key) {
            errors.push(FieldError::new("request_key", "invalid"));
        }
        if !errors.is_empty() {
            return Err(CoreError::Validation(errors));
        }

        let tx = self.begin_write().await?;
        let order = live_order(&tx, order_id).await?;
        // Повтор запроса: тот же человек, та же позиция и количество — прежний ответ без
        // нового списания; иначе ключ занят другим списанием.
        if let Some(row) = order_part::Entity::find()
            .filter(order_part::Column::OrderId.eq(order_id))
            .filter(order_part::Column::RequestKey.eq(&new.request_key))
            .one(&tx)
            .await?
        {
            if row.stock_item_id != Some(new.stock_item_id)
                || row.quantity != new.quantity
                || row.added_by_id != actor.user_id()
            {
                return Err(validation("request_key", "duplicate"));
            }
            let parts = order_parts(&tx, order_id, true).await?;
            tx.commit().await?;
            return Ok(parts);
        }

        let item = stock_item::Entity::find_by_id(new.stock_item_id)
            .one(&tx)
            .await?
            .ok_or(CoreError::NotFound {
                entity: ENTITY,
                id: new.stock_item_id,
            })?;
        if item.deleted_at.is_some() {
            return Err(validation("stock_item_id", "deleted"));
        }
        let now = self.now();
        let res = stock_item::Entity::update_many()
            .col_expr(
                stock_item::Column::Quantity,
                Expr::col(stock_item::Column::Quantity).sub(new.quantity),
            )
            .col_expr(
                stock_item::Column::Version,
                Expr::col(stock_item::Column::Version).add(1),
            )
            .col_expr(stock_item::Column::UpdatedAt, Expr::value(now))
            .filter(stock_item::Column::Id.eq(item.id))
            .filter(stock_item::Column::DeletedAt.is_null())
            .filter(stock_item::Column::Quantity.gte(new.quantity))
            .exec(&tx)
            .await?;
        // Позиция проверена живой в этой же транзакции: ноль строк — только нехватка.
        if res.rows_affected == 0 {
            return Err(validation("quantity", "not_enough_stock"));
        }

        let reduced = reduce_places(&tx, item.id, item.quantity - new.quantity).await?;
        let name = display_name(&item);
        let sum = part_sum(Money::from_kopecks(item.price), new.quantity).ok_or_else(overflow)?;
        let row = order_part::ActiveModel {
            id: NotSet,
            order_id: Set(order_id),
            stock_item_id: Set(Some(item.id)),
            name: Set(name.clone()),
            price: Set(item.price),
            quantity: Set(new.quantity),
            added_by_id: Set(actor.user_id()),
            added_by_name: Set(actor.name().to_owned()),
            added_at: Set(now),
            legacy: Set(false),
            request_key: Set(Some(new.request_key)),
        }
        .insert(&tx)
        .await?;
        let mut summary = format!(
            "Списано со склада: {name} × {} — {}",
            new.quantity,
            self.money_text(sum, MoneyText::Short)
        );
        let mut details = json!({
            "part_id": row.id,
            "quantity": new.quantity,
            "price": item.price,
        });
        if !reduced.is_empty() {
            summary.push_str(&reduced_tail(&reduced));
            details["places_reduced"] = reduced_json(&reduced);
        }
        write_audit(
            &tx,
            now,
            actor,
            NewAuditEntry {
                action: "order.part_added",
                order_id: Some(order_id),
                client_id: Some(order.client_id),
                stock_item_id: Some(item.id),
                summary,
                details: Some(details),
            },
        )
        .await?;
        let parts = order_parts(&tx, order_id, true).await?;
        tx.commit().await?;
        Ok(parts)
    }

    /// Возврат запчасти заказа (права `orders_edit` и `stock_view`): строка снимается; позиция
    /// жива — остаток растёт на списанное, её версия тоже; позиции нет (удалена или не было уже
    /// в MyWork 2) — только снятие строки. Строка другого заказа или уже возвращённая — «не
    /// найдена». Версия заказа не меняется. Возвращает оставшиеся запчасти заказа.
    pub async fn return_order_part(
        &self,
        actor: &Actor,
        order_id: i64,
        part_id: i64,
    ) -> Result<OrderParts, CoreError> {
        require_write_off(actor)?;
        let tx = self.begin_write().await?;
        let order = live_order(&tx, order_id).await?;
        let row = order_part::Entity::find_by_id(part_id)
            .filter(order_part::Column::OrderId.eq(order_id))
            .one(&tx)
            .await?
            .ok_or(CoreError::NotFound {
                entity: PART_ENTITY,
                id: part_id,
            })?;
        order_part::Entity::delete_by_id(row.id).exec(&tx).await?;

        let now = self.now();
        let restocked = match row.stock_item_id {
            Some(item_id) => {
                stock_item::Entity::update_many()
                    .col_expr(
                        stock_item::Column::Quantity,
                        Expr::col(stock_item::Column::Quantity).add(row.quantity),
                    )
                    .col_expr(
                        stock_item::Column::Version,
                        Expr::col(stock_item::Column::Version).add(1),
                    )
                    .col_expr(stock_item::Column::UpdatedAt, Expr::value(now))
                    .filter(stock_item::Column::Id.eq(item_id))
                    .filter(stock_item::Column::DeletedAt.is_null())
                    .exec(&tx)
                    .await?
                    .rows_affected
                    > 0
            }
            None => false,
        };

        let sum = part_sum(Money::from_kopecks(row.price), row.quantity).ok_or_else(overflow)?;
        let what = format!(
            "{} × {} — {}",
            row.name,
            row.quantity,
            self.money_text(sum, MoneyText::Short)
        );
        let summary = if restocked {
            format!("Возвращено на склад: {what}")
        } else {
            format!("Снято с заказа, позиции на складе нет: {what}")
        };
        write_audit(
            &tx,
            now,
            actor,
            NewAuditEntry {
                action: "order.part_returned",
                order_id: Some(order_id),
                client_id: Some(order.client_id),
                stock_item_id: row.stock_item_id,
                summary,
                details: Some(json!({
                    "part_id": row.id,
                    "quantity": row.quantity,
                    "price": row.price,
                    "restocked": restocked,
                })),
            },
        )
        .await?;
        let parts = order_parts(&tx, order_id, true).await?;
        tx.commit().await?;
        Ok(parts)
    }
}
