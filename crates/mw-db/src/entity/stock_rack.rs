use sea_orm::entity::prelude::*;

/// Шкаф склада: `code` — заглавные латинские и цифры, `shelves` × `cells_per_shelf` ячеек.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "stock_racks")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub code: String,
    pub label: String,
    pub shelves: i32,
    pub cells_per_shelf: i32,
    pub service_point_id: Option<i64>,
    pub sort_order: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
