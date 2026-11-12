use sea_orm::entity::prelude::*;

/// След строки старой базы: откуда пришла, куда попала, исходные значения (`raw` — JSON).
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "legacy_rows")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub import_id: i64,
    pub source: String,
    pub source_key: String,
    pub target_table: Option<String>,
    pub target_id: Option<i64>,
    pub raw: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
