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
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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

/// Заказ (все колонки, кроме `search_text`). Номер заказа = `id`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
