use chrono::NaiveDateTime;
use mw_db::entity::order;
use mw_domain::money::Money;
use mw_domain::search::index_text;
use serde::{Deserialize, Serialize};

use crate::NewClient;

/// Клиент заказа: существующий или новый (создаётся в той же транзакции).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClientRef {
    Existing(i64),
    New(NewClient),
}

impl Default for ClientRef {
    fn default() -> Self {
        Self::New(NewClient::default())
    }
}

/// Данные приёма устройства.
#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NewOrder {
    pub client: ClientRef,
    pub service_point_id: Option<i64>,
    pub status_id: Option<i64>,
    pub device_type: String,
    pub brand: String,
    pub model: String,
    pub serial: String,
    pub color: String,
    pub device_password: String,
    pub completeness: String,
    pub device_condition: String,
    pub malfunction: String,
    pub internal_comment: String,
    pub master: String,
    pub estimate_from: Option<Money>,
    pub estimate_to: Option<Money>,
    pub prepayment: Money,
    pub warranty_days: Option<u16>,
    pub need_call: bool,
    pub need_parts: bool,
}

/// Частичная правка заказа: `None` — поле не меняется; `Some(None)` очищает необязательное поле.
#[derive(Clone, Default, PartialEq)]
pub struct OrderPatch {
    pub client_id: Option<i64>,
    pub service_point_id: Option<Option<i64>>,
    pub received_at: Option<NaiveDateTime>,
    pub issued_at: Option<Option<NaiveDateTime>>,
    pub device_type: Option<String>,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub color: Option<String>,
    pub device_password: Option<String>,
    pub completeness: Option<String>,
    pub device_condition: Option<String>,
    pub malfunction: Option<String>,
    pub work_done: Option<String>,
    pub internal_comment: Option<String>,
    pub master: Option<String>,
    pub estimate_from: Option<Option<Money>>,
    pub estimate_to: Option<Option<Money>>,
    pub final_price: Option<Option<Money>>,
    pub prepayment: Option<Money>,
    pub extra_costs: Option<Money>,
    pub discount: Option<Money>,
    pub warranty_days: Option<Option<u16>>,
    pub need_call: Option<bool>,
    pub need_parts: Option<bool>,
}

/// Заказ (все колонки, кроме `search_text`). Номер заказа = `id`.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Order {
    pub id: i64,
    pub client_id: i64,
    pub service_point_id: Option<i64>,
    pub status_id: i64,
    pub received_at: NaiveDateTime,
    pub issued_at: Option<NaiveDateTime>,
    pub prepaid_at: Option<NaiveDateTime>,
    pub device_type: String,
    pub brand: String,
    pub model: String,
    pub serial: String,
    pub color: String,
    pub device_password: String,
    pub completeness: String,
    pub device_condition: String,
    pub malfunction: String,
    pub work_done: String,
    pub internal_comment: String,
    pub master: String,
    pub barcode: String,
    pub estimate_from: Option<Money>,
    pub estimate_to: Option<Money>,
    pub final_price: Option<Money>,
    pub prepayment: Money,
    pub extra_costs: Money,
    pub discount: Money,
    pub warranty_days: Option<u16>,
    pub need_call: bool,
    pub need_parts: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub deleted_at: Option<NaiveDateTime>,
    pub version: i64,
}

impl From<order::Model> for Order {
    fn from(m: order::Model) -> Self {
        let money = |v: i64| Money::from_kopecks(v);
        Self {
            id: m.id,
            client_id: m.client_id,
            service_point_id: m.service_point_id,
            status_id: m.status_id,
            received_at: m.received_at,
            issued_at: m.issued_at,
            prepaid_at: m.prepaid_at,
            device_type: m.device_type,
            brand: m.brand,
            model: m.model,
            serial: m.serial,
            color: m.color,
            device_password: m.device_password,
            completeness: m.completeness,
            device_condition: m.device_condition,
            malfunction: m.malfunction,
            work_done: m.work_done,
            internal_comment: m.internal_comment,
            master: m.master,
            barcode: m.barcode,
            estimate_from: m.estimate_from.map(money),
            estimate_to: m.estimate_to.map(money),
            final_price: m.final_price.map(money),
            prepayment: money(m.prepayment),
            extra_costs: money(m.extra_costs),
            discount: money(m.discount),
            warranty_days: m.warranty_days.and_then(|d| u16::try_from(d).ok()),
            need_call: m.need_call,
            need_parts: m.need_parts,
            created_at: m.created_at,
            updated_at: m.updated_at,
            deleted_at: m.deleted_at,
            version: m.version,
        }
    }
}

/// Пароль устройства в логи не попадает.
impl std::fmt::Debug for NewOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NewOrder")
            .field("client", &self.client)
            .field("service_point_id", &self.service_point_id)
            .field("status_id", &self.status_id)
            .field("device_type", &self.device_type)
            .field("brand", &self.brand)
            .field("model", &self.model)
            .field("serial", &self.serial)
            .field("color", &self.color)
            .field("device_password", &"***")
            .field("completeness", &self.completeness)
            .field("device_condition", &self.device_condition)
            .field("malfunction", &self.malfunction)
            .field("internal_comment", &self.internal_comment)
            .field("master", &self.master)
            .field("estimate_from", &self.estimate_from)
            .field("estimate_to", &self.estimate_to)
            .field("prepayment", &self.prepayment)
            .field("warranty_days", &self.warranty_days)
            .field("need_call", &self.need_call)
            .field("need_parts", &self.need_parts)
            .finish()
    }
}

/// Пароль устройства в логи не попадает.
impl std::fmt::Debug for OrderPatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OrderPatch")
            .field("client_id", &self.client_id)
            .field("service_point_id", &self.service_point_id)
            .field("received_at", &self.received_at)
            .field("issued_at", &self.issued_at)
            .field("device_type", &self.device_type)
            .field("brand", &self.brand)
            .field("model", &self.model)
            .field("serial", &self.serial)
            .field("color", &self.color)
            .field(
                "device_password",
                &self.device_password.as_ref().map(|_| "***"),
            )
            .field("completeness", &self.completeness)
            .field("device_condition", &self.device_condition)
            .field("malfunction", &self.malfunction)
            .field("work_done", &self.work_done)
            .field("internal_comment", &self.internal_comment)
            .field("master", &self.master)
            .field("estimate_from", &self.estimate_from)
            .field("estimate_to", &self.estimate_to)
            .field("final_price", &self.final_price)
            .field("prepayment", &self.prepayment)
            .field("extra_costs", &self.extra_costs)
            .field("discount", &self.discount)
            .field("warranty_days", &self.warranty_days)
            .field("need_call", &self.need_call)
            .field("need_parts", &self.need_parts)
            .finish()
    }
}

/// Пароль устройства в логи не попадает.
impl std::fmt::Debug for Order {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Order")
            .field("id", &self.id)
            .field("client_id", &self.client_id)
            .field("service_point_id", &self.service_point_id)
            .field("status_id", &self.status_id)
            .field("received_at", &self.received_at)
            .field("issued_at", &self.issued_at)
            .field("prepaid_at", &self.prepaid_at)
            .field("device_type", &self.device_type)
            .field("brand", &self.brand)
            .field("model", &self.model)
            .field("serial", &self.serial)
            .field("color", &self.color)
            .field("device_password", &"***")
            .field("completeness", &self.completeness)
            .field("device_condition", &self.device_condition)
            .field("malfunction", &self.malfunction)
            .field("work_done", &self.work_done)
            .field("internal_comment", &self.internal_comment)
            .field("master", &self.master)
            .field("barcode", &self.barcode)
            .field("estimate_from", &self.estimate_from)
            .field("estimate_to", &self.estimate_to)
            .field("final_price", &self.final_price)
            .field("prepayment", &self.prepayment)
            .field("extra_costs", &self.extra_costs)
            .field("discount", &self.discount)
            .field("warranty_days", &self.warranty_days)
            .field("need_call", &self.need_call)
            .field("need_parts", &self.need_parts)
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .field("deleted_at", &self.deleted_at)
            .field("version", &self.version)
            .finish()
    }
}

/// Текст для поиска по заказу (без данных клиента: поиск соединяет таблицы).
pub(crate) fn order_search_text(m: &order::Model) -> String {
    index_text(&[
        &m.id.to_string(),
        &m.device_type,
        &m.brand,
        &m.model,
        &m.serial,
        &m.barcode,
        &m.malfunction,
        &m.color,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "s3cr3t-pass-9137";

    #[test]
    fn debug_output_masks_device_password() {
        let new = NewOrder {
            device_password: SECRET.into(),
            brand: "Samsung".into(),
            ..Default::default()
        };
        let patch = OrderPatch {
            device_password: Some(SECRET.into()),
            brand: Some("Apple".into()),
            ..Default::default()
        };
        let order = Order::from(order::Model {
            id: 1,
            client_id: 1,
            service_point_id: None,
            status_id: 1,
            received_at: NaiveDateTime::default(),
            issued_at: None,
            prepaid_at: None,
            device_type: String::new(),
            brand: "Nokia".into(),
            model: String::new(),
            serial: String::new(),
            color: String::new(),
            device_password: SECRET.into(),
            completeness: String::new(),
            device_condition: String::new(),
            malfunction: String::new(),
            work_done: String::new(),
            internal_comment: String::new(),
            master: String::new(),
            barcode: String::new(),
            estimate_from: None,
            estimate_to: None,
            final_price: None,
            prepayment: 0,
            extra_costs: 0,
            discount: 0,
            warranty_days: None,
            need_call: false,
            need_parts: false,
            created_at: NaiveDateTime::default(),
            updated_at: NaiveDateTime::default(),
            deleted_at: None,
            version: 1,
            search_text: String::new(),
        });
        for (text, brand) in [
            (format!("{new:?}"), "Samsung"),
            (format!("{patch:?}"), "Apple"),
            (format!("{order:?}"), "Nokia"),
        ] {
            assert!(!text.contains(SECRET), "пароль попал в Debug: {text}");
            assert!(text.contains("\"***\""), "{text}");
            assert!(text.contains(brand), "{text}");
        }
    }
}
