//! Платежи по заказу: список, приём и удаление.
//!
//! Платёж и пересчёт `orders.prepayment` идут одной транзакцией `begin_write()`. Версия заказа и
//! `updated_at` не меняются (как у запчастей). `orders.prepayment` — сумма платежей этапа
//! «до выдачи»; `paid` — сумма всех платежей. Складывает копейки одна функция, `sum_kopecks`.

use std::collections::HashMap;

use chrono::{NaiveDate, NaiveDateTime};
use mw_db::entity::{order, order_payment, status};
use mw_domain::money::Money;
use mw_domain::payment::{MAX_RECEIVED_BY_CHARS, PaymentStage, amount_in_range};
use mw_domain::permission::Permission;
use mw_domain::status::StatusKind;
use mw_domain::validation::FieldError;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseTransaction, EntityTrait, NotSet,
    QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::update::{deleted_error, load};
use crate::attachments::valid_key;
use crate::export::dmy;
use crate::money_text::MoneyText;
use crate::sql::ID_CHUNK;
use crate::{Actor, Core, CoreError, NewAuditEntry, write_audit};

/// Название платежа в ошибке «не найдено».
pub(crate) const PAYMENT_ENTITY: &str = "платёж заказа";

/// Платёж по заказу.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct OrderPayment {
    /// Номер платежа.
    pub id: i64,
    /// День оплаты.
    pub day: NaiveDate,
    /// Сумма, в копейках.
    pub amount: Money,
    /// Кто принял (текст; может быть пустым).
    pub received_by: String,
    /// Этап относительно выдачи.
    pub stage: PaymentStage,
    /// Кто внёс запись (имя на момент внесения).
    pub added_by_name: String,
    /// Когда внесена (местное время предприятия).
    pub added_at: NaiveDateTime,
    /// Перенесён из предоплаты заказа или из MyWork 2.
    pub legacy: bool,
}

/// Платежи заказа по дню и номеру и суммы.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct OrderPayments {
    /// Платежи по дню, затем по номеру.
    pub payments: Vec<OrderPayment>,
    /// Все платежи, в копейках.
    pub paid: Money,
    /// Платежи до выдачи (равна предоплате заказа), в копейках.
    pub prepayment: Money,
}

/// Приём платежа.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewOrderPayment {
    /// День оплаты: не раньше дня приёма заказа и не позже сегодняшнего.
    pub day: NaiveDate,
    /// Сумма, в копейках: от 1 копейки до 999 999 999,99.
    pub amount: Money,
    /// Кто принял; до 100 знаков, может быть пустым.
    pub received_by: String,
    /// Ключ повтора запроса: от 1 до 64 латинских букв, цифр и дефисов. Повтор с тем же ключом и
    /// тем же содержимым возвращает платежи без нового платежа.
    pub request_key: String,
}

/// Строка платежа для вставки.
pub(crate) struct PaymentRow {
    pub day: NaiveDate,
    pub amount: Money,
    pub received_by: String,
    pub stage: PaymentStage,
    pub legacy: bool,
    pub request_key: Option<String>,
}

fn validation(field: &str, code: &str) -> CoreError {
    CoreError::Validation(vec![FieldError::new(field, code)])
}

/// Сумма копеек; переполнение при пределах платежа недостижимо.
pub(crate) fn sum_kopecks(amounts: impl IntoIterator<Item = i64>) -> Result<Money, CoreError> {
    let mut total: i64 = 0;
    for a in amounts {
        total = total.checked_add(a).ok_or_else(|| {
            CoreError::Internal("сумма платежей заказа не помещается в число".into())
        })?;
    }
    Ok(Money::from_kopecks(total))
}

/// Сколько уплачено по каждому заказу из списка; заказа без платежей в карте нет. Список и
/// документы возьмут её в задаче 5.
#[allow(dead_code)]
pub(crate) async fn paid_by_order(
    conn: &impl ConnectionTrait,
    ids: &[i64],
) -> Result<HashMap<i64, Money>, CoreError> {
    let mut by_order: HashMap<i64, Vec<i64>> = HashMap::new();
    for chunk in ids.chunks(ID_CHUNK) {
        let rows: Vec<(i64, i64)> = order_payment::Entity::find()
            .filter(order_payment::Column::OrderId.is_in(chunk.iter().copied()))
            .all(conn)
            .await?
            .into_iter()
            .map(|p| (p.order_id, p.amount))
            .collect();
        for (order_id, amount) in rows {
            by_order.entry(order_id).or_default().push(amount);
        }
    }
    by_order
        .into_iter()
        .map(|(id, amounts)| Ok((id, sum_kopecks(amounts)?)))
        .collect()
}

/// Кто принял по умолчанию: имя пользователя; без входа (открытый режим) — пусто.
pub(crate) fn received_by_of(actor: &Actor) -> String {
    match actor.user_id() {
        Some(_) => actor.name().to_owned(),
        None => String::new(),
    }
}

pub(crate) async fn insert_payment(
    tx: &DatabaseTransaction,
    now: NaiveDateTime,
    actor: &Actor,
    order_id: i64,
    row: PaymentRow,
) -> Result<order_payment::Model, CoreError> {
    Ok(order_payment::ActiveModel {
        id: NotSet,
        order_id: Set(order_id),
        day: Set(row.day),
        amount: Set(row.amount.kopecks()),
        received_by: Set(row.received_by),
        stage: Set(row.stage.as_str().to_owned()),
        added_by_id: Set(actor.user_id()),
        added_by_name: Set(actor.name().to_owned()),
        added_at: Set(now),
        legacy: Set(row.legacy),
        request_key: Set(row.request_key),
    }
    .insert(tx)
    .await?)
}

/// Пересчитывает `orders.prepayment` как сумму платежей до выдачи; версию не трогает.
pub(crate) async fn recount_prepayment(
    tx: &DatabaseTransaction,
    order_id: i64,
) -> Result<Money, CoreError> {
    let sum = sum_kopecks(
        order_payment::Entity::find()
            .filter(order_payment::Column::OrderId.eq(order_id))
            .filter(order_payment::Column::Stage.eq(PaymentStage::BeforeIssue.as_str()))
            .all(tx)
            .await?
            .into_iter()
            .map(|p| p.amount),
    )?;
    order::Entity::update_many()
        .col_expr(order::Column::Prepayment, Expr::value(sum.kopecks()))
        .filter(order::Column::Id.eq(order_id))
        .exec(tx)
        .await?;
    Ok(sum)
}

/// Подробности записи журнала о платеже.
pub(crate) fn payment_details(m: &order_payment::Model) -> serde_json::Value {
    json!({
        "payment_id": m.id,
        "day": m.day.to_string(),
        "amount": m.amount,
        "received_by": m.received_by,
        "stage": m.stage,
        "legacy": m.legacy,
    })
}

/// Платежи заказа по дню и номеру, с суммами.
pub(crate) async fn order_payments(
    conn: &impl ConnectionTrait,
    order_id: i64,
) -> Result<OrderPayments, CoreError> {
    let rows = order_payment::Entity::find()
        .filter(order_payment::Column::OrderId.eq(order_id))
        .order_by_asc(order_payment::Column::Day)
        .order_by_asc(order_payment::Column::Id)
        .all(conn)
        .await?;
    let paid = sum_kopecks(rows.iter().map(|r| r.amount))?;
    let prepayment = sum_kopecks(
        rows.iter()
            .filter(|r| r.stage == PaymentStage::BeforeIssue.as_str())
            .map(|r| r.amount),
    )?;
    let payments = rows
        .into_iter()
        .map(|r| {
            let stage = r.stage.parse::<PaymentStage>().map_err(|_| {
                CoreError::Internal(format!("неизвестный этап платежа «{}»", r.stage))
            })?;
            Ok(OrderPayment {
                id: r.id,
                day: r.day,
                amount: Money::from_kopecks(r.amount),
                received_by: r.received_by,
                stage,
                added_by_name: r.added_by_name,
                added_at: r.added_at,
                legacy: r.legacy,
            })
        })
        .collect::<Result<Vec<_>, CoreError>>()?;
    Ok(OrderPayments {
        payments,
        paid,
        prepayment,
    })
}

async fn live_order(tx: &DatabaseTransaction, id: i64) -> Result<order::Model, CoreError> {
    let order = load(tx, id).await?;
    if order.deleted_at.is_some() {
        return Err(deleted_error());
    }
    Ok(order)
}

impl Core {
    /// Платежи заказа. Права — как у самого заказа (достаточно войти); удалённый заказ читается.
    pub async fn list_order_payments(
        &self,
        _actor: &Actor,
        order_id: i64,
    ) -> Result<OrderPayments, CoreError> {
        load(&self.db, order_id).await?;
        order_payments(&self.db, order_id).await
    }

    /// Приём платежа (право `orders_edit`). День — от дня приёма заказа до сегодняшнего; этап —
    /// «после выдачи» у выданного заказа, иначе «до выдачи». Версия заказа не меняется.
    pub async fn add_order_payment(
        &self,
        actor: &Actor,
        order_id: i64,
        new: NewOrderPayment,
    ) -> Result<OrderPayments, CoreError> {
        actor.require(Permission::OrdersEdit)?;
        let received_by = new.received_by.trim().to_owned();
        let mut errors = Vec::new();
        if !amount_in_range(new.amount) {
            errors.push(FieldError::new("amount", "out_of_range"));
        }
        if received_by.chars().count() > MAX_RECEIVED_BY_CHARS {
            errors.push(FieldError::new("received_by", "too_long"));
        }
        if !valid_key(&new.request_key) {
            errors.push(FieldError::new("request_key", "invalid"));
        }
        if !errors.is_empty() {
            return Err(CoreError::Validation(errors));
        }

        let tx = self.begin_write().await?;
        let order = live_order(&tx, order_id).await?;
        let now = self.now();
        if new.day < order.received_at.date() || new.day > now.date() {
            return Err(validation("day", "payment_day"));
        }
        if let Some(row) = order_payment::Entity::find()
            .filter(order_payment::Column::OrderId.eq(order_id))
            .filter(order_payment::Column::RequestKey.eq(&new.request_key))
            .one(&tx)
            .await?
        {
            if row.day != new.day
                || row.amount != new.amount.kopecks()
                || row.received_by != received_by
                || row.added_by_id != actor.user_id()
            {
                return Err(validation("request_key", "duplicate"));
            }
            let payments = order_payments(&tx, order_id).await?;
            tx.commit().await?;
            return Ok(payments);
        }

        let issued = status::Entity::find_by_id(order.status_id)
            .one(&tx)
            .await?
            .is_some_and(|s| s.kind == StatusKind::Issued.as_str());
        let stage = if issued {
            PaymentStage::AfterIssue
        } else {
            PaymentStage::BeforeIssue
        };
        let row = insert_payment(
            &tx,
            now,
            actor,
            order_id,
            PaymentRow {
                day: new.day,
                amount: new.amount,
                received_by,
                stage,
                legacy: false,
                request_key: Some(new.request_key),
            },
        )
        .await?;
        recount_prepayment(&tx, order_id).await?;
        write_audit(
            &tx,
            now,
            actor,
            NewAuditEntry {
                action: "order.payment_added",
                order_id: Some(order_id),
                client_id: Some(order.client_id),
                stock_item_id: None,
                summary: self.payment_summary("Принята оплата", &row),
                details: Some(payment_details(&row)),
            },
        )
        .await?;
        let payments = order_payments(&tx, order_id).await?;
        tx.commit().await?;
        Ok(payments)
    }

    /// Удаление платежа (право `orders_edit`). Платёж другого заказа или уже удалённый — «не
    /// найден». Версия заказа не меняется.
    pub async fn remove_order_payment(
        &self,
        actor: &Actor,
        order_id: i64,
        payment_id: i64,
    ) -> Result<OrderPayments, CoreError> {
        actor.require(Permission::OrdersEdit)?;
        let tx = self.begin_write().await?;
        let order = live_order(&tx, order_id).await?;
        let row = order_payment::Entity::find_by_id(payment_id)
            .filter(order_payment::Column::OrderId.eq(order_id))
            .one(&tx)
            .await?
            .ok_or(CoreError::NotFound {
                entity: PAYMENT_ENTITY,
                id: payment_id,
            })?;
        order_payment::Entity::delete_by_id(row.id)
            .exec(&tx)
            .await?;
        recount_prepayment(&tx, order_id).await?;
        write_audit(
            &tx,
            self.now(),
            actor,
            NewAuditEntry {
                action: "order.payment_removed",
                order_id: Some(order_id),
                client_id: Some(order.client_id),
                stock_item_id: None,
                summary: self.payment_summary("Удалён платёж", &row),
                details: Some(payment_details(&row)),
            },
        )
        .await?;
        let payments = order_payments(&tx, order_id).await?;
        tx.commit().await?;
        Ok(payments)
    }

    /// «Принята оплата 1 500 ₽ (11.11.2026) · Иванов».
    fn payment_summary(&self, lead: &str, row: &order_payment::Model) -> String {
        let mut s = format!(
            "{lead} {} ({})",
            self.money_text(Money::from_kopecks(row.amount), MoneyText::Short),
            dmy(row.day)
        );
        if !row.received_by.is_empty() {
            s.push_str(" · ");
            s.push_str(&row.received_by);
        }
        s
    }
}
