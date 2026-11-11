use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "clients")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub full_name: String,
    pub phone: String,
    pub phone_key: String,
    pub address: String,
    pub note: String,
    pub source: String,
    pub blacklisted: bool,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub deleted_at: Option<DateTime>,
    pub version: i64,
    pub search_text: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
