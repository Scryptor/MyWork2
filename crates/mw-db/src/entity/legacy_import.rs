use sea_orm::entity::prelude::*;

/// Один запуск импорта из MyWork 2; `mode` — `into_empty` или `replace`, `report` — JSON.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "legacy_imports")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub imported_at: DateTime,
    pub source_path: String,
    pub mode: String,
    pub report: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
