use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "orders")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub client_id: i64,
    pub service_point_id: Option<i64>,
    pub status_id: i64,
    pub received_at: DateTime,
    pub issued_at: Option<DateTime>,
    pub prepaid_at: Option<DateTime>,
    pub device_type: String,
    pub brand: String,
    pub model: String,
    pub serial: String,
    pub color: String,
    pub device_password: String,
    pub completeness: String,
    pub device_condition: String,
    pub malfunction: String,
    pub work_done: String,
    pub internal_comment: String,
    pub master: String,
    pub barcode: String,
    pub estimate_from: Option<i64>,
    pub estimate_to: Option<i64>,
    pub final_price: Option<i64>,
    pub prepayment: i64,
    pub extra_costs: i64,
    pub discount: i64,
    pub warranty_days: Option<i32>,
    pub warranty_text: String,
    pub warranty_months: Option<i32>,
    pub need_call: bool,
    pub need_parts: bool,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub deleted_at: Option<DateTime>,
    pub version: i64,
    pub search_text: String,
    pub status_changed_at: Option<DateTime>,
    pub device_type_key: String,
    pub brand_key: String,
    pub model_key: String,
    pub master_key: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
