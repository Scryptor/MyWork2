use sea_orm::entity::prelude::*;

/// Платёж по заказу: сумма, день оплаты и этап относительно выдачи.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "order_payments")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub order_id: i64,
    /// День оплаты.
    pub day: Date,
    /// Сумма, копейки.
    pub amount: i64,
    /// Кто принял платёж (текст).
    pub received_by: String,
    /// Код `mw_domain::payment::PaymentStage`.
    pub stage: String,
    pub added_by_id: Option<i64>,
    pub added_by_name: String,
    pub added_at: DateTime,
    /// Перенесён из предоплаты заказа или из MyWork 2.
    pub legacy: bool,
    /// Ключ повтора запроса; у перенесённых — `None`.
    pub request_key: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
