use sea_orm::entity::prelude::*;

/// Шаблон документа, изменённый предприятием; ключ — вид документа (`DocumentKind::as_str`).
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "document_templates")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub kind: String,
    /// JSON модели шаблона; `NULL` — стандартный.
    pub model: Option<String>,
    pub version: i64,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
