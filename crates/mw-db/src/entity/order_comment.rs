use sea_orm::entity::prelude::*;

/// Запись ленты заказа с именем автора на момент добавления.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "order_comments")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub order_id: i64,
    pub request_key: String,
    pub author_id: Option<i64>,
    pub author_name: String,
    pub body: String,
    pub created_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
