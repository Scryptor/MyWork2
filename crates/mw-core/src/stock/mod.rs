//! Склад: позиции (`items`) — типы API, проверки и выводимое наименование; фото позиций
//! (`photos`); запчасти заказа — списание и возврат (`parts`).

mod items;
mod parts;
mod photos;
mod racks;

pub use parts::{NewOrderPart, OrderPart, OrderParts};
pub use racks::{NewStockRack, StockCell, StockRack, StockRackPatch};

use chrono::NaiveDateTime;
use mw_db::entity::stock_item;
use mw_domain::money::Money;
use mw_domain::stock::{MAX_STOCK_PRICE, MAX_STOCK_QUANTITY, StockNameParts, needs_restock};
use mw_domain::validation::FieldError;
use serde::{Deserialize, Serialize};

use crate::CoreError;

/// Название сущности в ошибках «не найдено» и «изменена другим пользователем».
const ENTITY: &str = "позиция склада";

/// Наибольшая длина наименования и каждого из пяти полей позиции, знаков.
const MAX_TEXT_CHARS: usize = 200;
/// Наибольшая длина примечания позиции, знаков.
const MAX_NOTE_CHARS: usize = 2000;

/// Позиция склада.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct StockItem {
    /// Номер позиции («Арт.»).
    pub id: i64,
    /// Наименование, как его ввели; пусто — выводится из полей позиции.
    pub name: String,
    /// Наименование для показа: введённое, иначе «тип запчасти, тип устройства, бренд, модель,
    /// цвет», иначе «Позиция №…».
    pub display_name: String,
    /// Тип устройства.
    pub device_type: String,
    /// Тип запчасти.
    pub part_type: String,
    /// Бренд.
    pub brand: String,
    /// Модель.
    pub model: String,
    /// Цвет.
    pub color: String,
    /// Примечание.
    pub note: String,
    /// Остаток, штук.
    pub quantity: i64,
    /// Цена за штуку («сколько стоит вам»), в копейках.
    pub price: Money,
    /// «Докупить при»: порог остатка; 0 — не напоминать.
    pub restock_threshold: i64,
    /// Остаток ниже порога «Докупить при».
    pub needs_restock: bool,
    /// Когда добавлена (местное время предприятия).
    pub created_at: NaiveDateTime,
    /// Когда изменена (местное время предприятия).
    pub updated_at: NaiveDateTime,
    /// Версия записи для проверки конфликтов правок.
    pub version: i64,
}

impl From<stock_item::Model> for StockItem {
    fn from(m: stock_item::Model) -> Self {
        Self {
            display_name: display_name(&m),
            needs_restock: needs_restock(m.quantity, m.restock_threshold),
            id: m.id,
            name: m.name,
            device_type: m.device_type,
            part_type: m.part_type,
            brand: m.brand,
            model: m.model,
            color: m.color,
            note: m.note,
            quantity: m.quantity,
            price: Money::from_kopecks(m.price),
            restock_threshold: m.restock_threshold,
            created_at: m.created_at,
            updated_at: m.updated_at,
            version: m.version,
        }
    }
}

/// Новая позиция склада. Не переданное поле — пустое (ноль).
#[derive(
    Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema,
)]
#[serde(default, deny_unknown_fields)]
pub struct NewStockItem {
    /// Наименование; пусто — выводится из полей позиции.
    pub name: String,
    /// Тип устройства.
    pub device_type: String,
    /// Тип запчасти.
    pub part_type: String,
    /// Бренд.
    pub brand: String,
    /// Модель.
    pub model: String,
    /// Цвет.
    pub color: String,
    /// Примечание.
    pub note: String,
    /// Остаток, штук: 0…1 000 000.
    pub quantity: i64,
    /// Цена за штуку, в копейках: 0…10 000 000 ₽.
    pub price: Money,
    /// «Докупить при»: 0…1 000 000; 0 — не напоминать.
    pub restock_threshold: i64,
}

/// Частичная правка позиции склада; отсутствующее поле не меняется.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, ts_rs::TS, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
#[ts(optional_fields)]
pub struct StockItemPatch {
    /// Наименование; пустая строка — выводить из полей позиции.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub name: Option<String>,
    /// Тип устройства.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub device_type: Option<String>,
    /// Тип запчасти.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub part_type: Option<String>,
    /// Бренд.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub brand: Option<String>,
    /// Модель.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub model: Option<String>,
    /// Цвет.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub color: Option<String>,
    /// Примечание.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub note: Option<String>,
    /// Остаток, штук: 0…1 000 000.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub quantity: Option<i64>,
    /// Цена за штуку, в копейках: 0…10 000 000 ₽.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub price: Option<Money>,
    /// «Докупить при»: 0…1 000 000; 0 — не напоминать.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub restock_threshold: Option<i64>,
}

/// Поле сортировки списка склада.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum StockSortField {
    /// Номер позиции.
    Id,
    /// Выводимое наименование (по умолчанию).
    #[default]
    Name,
    /// Остаток.
    Quantity,
    /// Цена за штуку.
    Price,
}

/// Порядок списка склада; равные — по наименованию, затем по номеру.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StockSort {
    pub field: StockSortField,
    pub descending: bool,
}

/// Условия списка склада.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StockFilter {
    /// Слова поиска (все должны найтись); одно целое число находит ещё позицию с этим номером.
    /// Пусто — все позиции.
    pub query: String,
    /// Только «Докупить»: остаток ниже порога.
    pub restock: bool,
    pub sort: StockSort,
}

/// Сводка склада.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct StockSummary {
    /// Сколько позиций надо докупить (остаток ниже порога «Докупить при»).
    pub restock: u64,
}

/// Выводимое наименование позиции (`mw_domain::stock::display_name`).
pub(crate) fn display_name(m: &stock_item::Model) -> String {
    let parts = StockNameParts {
        part_type: &m.part_type,
        device_type: &m.device_type,
        brand: &m.brand,
        model: &m.model,
        color: &m.color,
    };
    mw_domain::stock::display_name(&m.name, &parts, m.id)
}

fn deleted_error() -> CoreError {
    CoreError::Validation(vec![FieldError::new("id", "deleted")])
}

/// Проверяет позицию: только поля, для которых `checked(поле)` — правка не мешает старым значениям
/// вне пределов, если их не трогает. Наименование обязательно, если тронуто хоть одно его поле.
fn validate(m: &stock_item::Model, checked: impl Fn(&str) -> bool) -> Result<(), CoreError> {
    let mut errors = Vec::new();
    // Поля наименования: если все пусты, позицию не назвать. Один список — и для
    // обязательности, и для длины.
    let name_fields = [
        ("name", &m.name),
        ("device_type", &m.device_type),
        ("part_type", &m.part_type),
        ("brand", &m.brand),
        ("model", &m.model),
        ("color", &m.color),
    ];
    if name_fields.iter().any(|(f, _)| checked(f))
        && name_fields.iter().all(|(_, v)| v.trim().is_empty())
    {
        errors.push(FieldError::required("name"));
    }
    for (field, value, max) in name_fields
        .iter()
        .map(|(f, v)| (*f, *v, MAX_TEXT_CHARS))
        .chain([("note", &m.note, MAX_NOTE_CHARS)])
    {
        if checked(field) && value.chars().count() > max {
            errors.push(FieldError::new(field, "too_long"));
        }
    }
    let quantity = 0..=MAX_STOCK_QUANTITY;
    for (field, value, range) in [
        ("quantity", m.quantity, quantity.clone()),
        ("price", m.price, 0..=MAX_STOCK_PRICE.kopecks()),
        ("restock_threshold", m.restock_threshold, quantity),
    ] {
        if checked(field) && !range.contains(&value) {
            errors.push(FieldError::new(field, "out_of_range"));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(CoreError::Validation(errors))
    }
}
