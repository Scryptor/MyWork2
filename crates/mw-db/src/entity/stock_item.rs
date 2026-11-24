use sea_orm::entity::prelude::*;

/// Позиция склада. `id` — номер позиции («Арт.»; у перенесённых из MyWork 2 — прежний), `price` —
/// цена за штуку в копейках, `restock_threshold` — «Докупить при» (0 — не напоминать).
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "stock_items")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// Наименование, как его ввёл человек; пусто — выводится из полей позиции.
    pub name: String,
    pub device_type: String,
    pub part_type: String,
    pub brand: String,
    pub model: String,
    pub color: String,
    pub note: String,
    pub quantity: i64,
    pub price: i64,
    pub restock_threshold: i64,
    pub search_text: String,
    /// `normalize_search` выводимого наименования — для сортировки.
    pub name_key: String,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub deleted_at: Option<DateTime>,
    pub version: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
