use chrono::NaiveDateTime;
use mw_db::entity::order;
use mw_domain::money::Money;
use mw_domain::search::index_text;
use serde::{Deserialize, Serialize};

use crate::NewClient;

/// Клиент заказа: существующий или новый (создаётся в той же транзакции).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ClientRef {
    /// Существующий клиент: его номер.
    Existing(i64),
    /// Новый клиент: данные для создания.
    New(NewClient),
}

impl Default for ClientRef {
    fn default() -> Self {
        Self::New(NewClient::default())
    }
}

/// Данные приёма устройства.
#[derive(Clone, Default, PartialEq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewOrder {
    /// Клиент заказа: существующий или новый.
    pub client: ClientRef,
    /// Номер точки обслуживания; пусто — не указана.
    #[serde(deserialize_with = "crate::patch::required_nullable")]
    #[schema(required = true)]
    pub service_point_id: Option<i64>,
    /// Номер статуса; пусто — статус по умолчанию.
    #[serde(deserialize_with = "crate::patch::required_nullable")]
    #[schema(required = true)]
    pub status_id: Option<i64>,
    /// Тип устройства.
    pub device_type: String,
    /// Бренд.
    pub brand: String,
    /// Модель.
    pub model: String,
    /// Серийный номер.
    pub serial: String,
    /// Цвет.
    pub color: String,
    /// Пароль устройства.
    pub device_password: String,
    /// Комплектность.
    pub completeness: String,
    /// Внешнее состояние устройства.
    pub device_condition: String,
    /// Неисправность со слов клиента.
    pub malfunction: String,
    /// Внутренний комментарий.
    pub internal_comment: String,
    /// Мастер.
    pub master: String,
    /// Оценка стоимости: нижняя граница, в копейках.
    #[serde(deserialize_with = "crate::patch::required_nullable")]
    #[schema(required = true)]
    pub estimate_from: Option<Money>,
    /// Оценка стоимости: верхняя граница, в копейках.
    #[serde(deserialize_with = "crate::patch::required_nullable")]
    #[schema(required = true)]
    pub estimate_to: Option<Money>,
    /// Предоплата, в копейках.
    pub prepayment: Money,
    /// Гарантия, в сутках; пусто — без гарантии.
    #[serde(deserialize_with = "crate::patch::required_nullable")]
    #[schema(required = true)]
    pub warranty_days: Option<u16>,
    /// Нужно позвонить клиенту.
    pub need_call: bool,
    /// Нужны запчасти.
    pub need_parts: bool,
}

/// Частичная правка заказа: `None` — поле не меняется; `Some(None)` очищает необязательное поле.
#[derive(Clone, Default, PartialEq, Deserialize, ts_rs::TS, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
#[ts(optional_fields)]
pub struct OrderPatch {
    /// Номер клиента.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub client_id: Option<i64>,
    /// Номер точки обслуживания; пусто — не указана.
    #[serde(default, deserialize_with = "crate::patch::nullable")]
    pub service_point_id: Option<Option<i64>>,
    /// Когда принят (местное время предприятия).
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub received_at: Option<NaiveDateTime>,
    /// Когда выдан (местное время предприятия); пусто, если не выдан.
    #[serde(default, deserialize_with = "crate::patch::nullable")]
    pub issued_at: Option<Option<NaiveDateTime>>,
    /// Тип устройства.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub device_type: Option<String>,
    /// Бренд.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub brand: Option<String>,
    /// Модель.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub model: Option<String>,
    /// Серийный номер.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub serial: Option<String>,
    /// Цвет.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub color: Option<String>,
    /// Пароль устройства.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub device_password: Option<String>,
    /// Комплектность.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub completeness: Option<String>,
    /// Внешнее состояние устройства.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub device_condition: Option<String>,
    /// Неисправность со слов клиента.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub malfunction: Option<String>,
    /// Выполненные работы.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub work_done: Option<String>,
    /// Внутренний комментарий.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub internal_comment: Option<String>,
    /// Мастер.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub master: Option<String>,
    /// Оценка стоимости: нижняя граница, в копейках.
    #[serde(default, deserialize_with = "crate::patch::nullable")]
    pub estimate_from: Option<Option<Money>>,
    /// Оценка стоимости: верхняя граница, в копейках.
    #[serde(default, deserialize_with = "crate::patch::nullable")]
    pub estimate_to: Option<Option<Money>>,
    /// Итоговая цена, в копейках.
    #[serde(default, deserialize_with = "crate::patch::nullable")]
    pub final_price: Option<Option<Money>>,
    /// Предоплата, в копейках.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub prepayment: Option<Money>,
    /// Дополнительные расходы, в копейках.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub extra_costs: Option<Money>,
    /// Скидка, в копейках.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub discount: Option<Money>,
    /// Гарантия, в сутках; пусто — без гарантии.
    #[serde(default, deserialize_with = "crate::patch::nullable")]
    pub warranty_days: Option<Option<u16>>,
    /// Нужно позвонить клиенту.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub need_call: Option<bool>,
    /// Нужны запчасти.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub need_parts: Option<bool>,
}

/// Заказ (все колонки, кроме `search_text`). Номер заказа = `id`.
#[derive(Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct Order {
    /// Номер заказа (он же номер квитанции).
    pub id: i64,
    /// Номер клиента.
    pub client_id: i64,
    /// Номер точки обслуживания; пусто — не указана.
    pub service_point_id: Option<i64>,
    /// Номер статуса.
    pub status_id: i64,
    /// Когда принят (местное время предприятия).
    pub received_at: NaiveDateTime,
    /// Когда выдан (местное время предприятия); пусто, если не выдан.
    pub issued_at: Option<NaiveDateTime>,
    /// Когда внесена предоплата (местное время предприятия).
    pub prepaid_at: Option<NaiveDateTime>,
    /// Тип устройства.
    pub device_type: String,
    /// Бренд.
    pub brand: String,
    /// Модель.
    pub model: String,
    /// Серийный номер.
    pub serial: String,
    /// Цвет.
    pub color: String,
    /// Пароль устройства.
    pub device_password: String,
    /// Комплектность.
    pub completeness: String,
    /// Внешнее состояние устройства.
    pub device_condition: String,
    /// Неисправность со слов клиента.
    pub malfunction: String,
    /// Выполненные работы.
    pub work_done: String,
    /// Внутренний комментарий.
    pub internal_comment: String,
    /// Мастер.
    pub master: String,
    /// Штрихкод заказа.
    pub barcode: String,
    /// Оценка стоимости: нижняя граница, в копейках.
    pub estimate_from: Option<Money>,
    /// Оценка стоимости: верхняя граница, в копейках.
    pub estimate_to: Option<Money>,
    /// Итоговая цена, в копейках.
    pub final_price: Option<Money>,
    /// Предоплата, в копейках.
    pub prepayment: Money,
    /// Дополнительные расходы, в копейках.
    pub extra_costs: Money,
    /// Скидка, в копейках.
    pub discount: Money,
    /// Гарантия, в сутках; пусто — без гарантии.
    pub warranty_days: Option<u16>,
    /// Нужно позвонить клиенту.
    pub need_call: bool,
    /// Нужны запчасти.
    pub need_parts: bool,
    /// Когда создано (местное время предприятия).
    pub created_at: NaiveDateTime,
    /// Когда изменено (местное время предприятия).
    pub updated_at: NaiveDateTime,
    /// Когда удалено (местное время предприятия); пусто, если не удалено.
    pub deleted_at: Option<NaiveDateTime>,
    /// Версия записи для проверки конфликтов правок.
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
