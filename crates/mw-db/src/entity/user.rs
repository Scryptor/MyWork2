use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub login: String,
    pub login_key: String,
    pub display_name: String,
    pub role_id: i64,
    pub password_hash: String,
    /// `argon2` или `legacy_sha1`.
    pub password_scheme: String,
    pub active: bool,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub version: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
