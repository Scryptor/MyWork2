use sea_orm::entity::prelude::*;

/// Место позиции: сколько штук лежит в ячейке шкафа.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "stock_places")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub stock_item_id: i64,
    pub rack_id: i64,
    pub shelf: i32,
    pub cell: i32,
    pub quantity: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
