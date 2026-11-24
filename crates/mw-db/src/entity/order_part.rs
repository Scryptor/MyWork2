use sea_orm::entity::prelude::*;

/// Запчасть заказа — списание со склада со снимком наименования и цены на момент списания.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "order_parts")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub order_id: i64,
    /// Позиция склада; `None` — позиции не было уже в MyWork 2.
    pub stock_item_id: Option<i64>,
    /// Выводимое наименование позиции на момент списания.
    pub name: String,
    /// Цена за штуку на момент списания, копейки.
    pub price: i64,
    pub quantity: i64,
    pub added_by_id: Option<i64>,
    pub added_by_name: String,
    pub added_at: DateTime,
    /// Перенесена из MyWork 2.
    pub legacy: bool,
    /// Ключ повтора запроса списания; у перенесённых — `None`.
    pub request_key: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
