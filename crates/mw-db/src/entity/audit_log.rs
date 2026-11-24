use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "audit_log")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub at: DateTime,
    pub user_id: Option<i64>,
    pub user_name: String,
    pub action: String,
    pub order_id: Option<i64>,
    pub client_id: Option<i64>,
    pub summary: String,
    /// JSON.
    pub details: Option<String>,
    pub stock_item_id: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
