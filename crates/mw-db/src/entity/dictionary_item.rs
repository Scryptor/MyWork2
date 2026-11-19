use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "dictionary_items")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub kind: String,
    pub value: String,
    pub sort_order: i32,
    pub archived: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
