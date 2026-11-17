use sea_orm::entity::prelude::*;

/// Вложение заказа: фото или файл. `path` и `thumb_path` — относительно папки данных.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "attachments")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub order_id: Option<i64>,
    pub draft_key: Option<String>,
    pub kind: String,
    pub name: String,
    pub caption: String,
    pub content_type: String,
    pub size: i64,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub path: String,
    pub thumb_path: Option<String>,
    pub legacy: bool,
    pub created_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
