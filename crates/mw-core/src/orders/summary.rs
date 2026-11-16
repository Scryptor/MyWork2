//! Сводка по заказам и подсказки значений для фильтров.

use std::collections::HashMap;

use mw_db::entity::{order, status};
use mw_domain::search::{like_contains, normalize_search};
use mw_domain::status::StatusKind;
use sea_orm::sea_query::{Asterisk, Condition, Expr, ExprTrait, Func, Order, Query, SimpleExpr};
use sea_orm::{ConnectionTrait, StatementBuilder};
use serde::{Deserialize, Serialize};

use super::list::{contains, o, st};
use crate::{Actor, Core, CoreError};

/// Сколько заказов в статусе.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct StatusCount {
    /// Номер статуса.
    pub status_id: i64,
    /// Сколько неудалённых заказов в этом статусе.
    pub count: u64,
}

/// Сводка по заказам для экрана списка.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct OrderSummary {
    /// Неудалённые заказы по статусам; статусы без заказов не включаются.
    pub by_status: Vec<StatusCount>,
    /// Дальше — только заказы в сервисе (вид статуса не «выдан»): нужно позвонить.
    pub need_call: u64,
    /// Нужны запчасти.
    pub need_parts: u64,
    /// Диагностика просрочена.
    pub overdue_diagnostics: u64,
    /// Давно готовы.
    pub stale_ready: u64,
}

/// Поле, значения которого предлагаются в подсказках.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum OrderValueField {
    /// Тип устройства.
    DeviceType,
    /// Бренд.
    Brand,
    /// Модель.
    Model,
    /// Мастер.
    Master,
}

/// Сколько значений отдают подсказки.
const VALUES_LIMIT: usize = 20;

impl Core {
    /// Сводка по неудалённым заказам; `service_point_id` сужает до точки.
    pub async fn order_summary(
        &self,
        _actor: &Actor,
        service_point_id: Option<i64>,
    ) -> Result<OrderSummary, CoreError> {
        let backend = self.db.get_database_backend();
        let mut cond = Condition::all().add(o(order::Column::DeletedAt).is_null());
        if let Some(id) = service_point_id {
            cond = cond.add(o(order::Column::ServicePointId).eq(id));
        }

        let mut groups = Query::select();
        groups
            .column(order::Column::StatusId)
            .expr(Func::count(Expr::col(Asterisk)))
            .from(order::Entity)
            .cond_where(cond.clone())
            .group_by_col(order::Column::StatusId)
            .order_by(order::Column::StatusId, Order::Asc);
        let mut by_status = Vec::new();
        for row in self
            .db
            .query_all_raw(StatementBuilder::build(&groups, &backend))
            .await?
        {
            let count = u64::try_from(row.try_get_by_index::<i64>(1)?).unwrap_or(0);
            by_status.push(StatusCount {
                status_id: row.try_get_by_index(0)?,
                count,
            });
        }

        // COUNT(CASE WHEN … THEN 1 END): пустое условие даёт NULL, который не считается.
        let counted = |c: SimpleExpr| -> SimpleExpr {
            Func::count(Expr::case(c, 1).finally(Expr::cust("NULL"))).into()
        };
        let mut attention = Query::select();
        attention
            .expr(counted(o(order::Column::NeedCall).eq(true)))
            .expr(counted(o(order::Column::NeedParts).eq(true)))
            .expr(counted(self.overdue_diagnostics_cond()))
            .expr(counted(self.stale_ready_cond()))
            .from(order::Entity)
            .inner_join(
                status::Entity,
                o(order::Column::StatusId).equals((status::Entity, status::Column::Id)),
            )
            .cond_where(cond.add(st(status::Column::Kind).ne(StatusKind::Issued.as_str())));
        let row = self
            .db
            .query_one_raw(StatementBuilder::build(&attention, &backend))
            .await?
            .ok_or_else(|| CoreError::Internal("пустой результат сводки".to_owned()))?;
        let n = |i: usize| -> Result<u64, CoreError> {
            Ok(u64::try_from(row.try_get_by_index::<i64>(i)?).unwrap_or(0))
        };
        Ok(OrderSummary {
            by_status,
            need_call: n(0)?,
            need_parts: n(1)?,
            overdue_diagnostics: n(2)?,
            stale_ready: n(3)?,
        })
    }

    /// Различные значения поля по ключу (без учёта регистра), самые частые сверху, не больше 20.
    /// Для каждого ключа — самое частое написание. `query` — подстрока ключа; `brand` сужает
    /// только модели.
    pub async fn order_values(
        &self,
        _actor: &Actor,
        field: OrderValueField,
        query: &str,
        brand: Option<&str>,
    ) -> Result<Vec<String>, CoreError> {
        let backend = self.db.get_database_backend();
        let (key_col, raw_col) = match field {
            OrderValueField::DeviceType => {
                (order::Column::DeviceTypeKey, order::Column::DeviceType)
            }
            OrderValueField::Brand => (order::Column::BrandKey, order::Column::Brand),
            OrderValueField::Model => (order::Column::ModelKey, order::Column::Model),
            OrderValueField::Master => (order::Column::MasterKey, order::Column::Master),
        };
        let mut cond = Condition::all()
            .add(o(order::Column::DeletedAt).is_null())
            .add(o(key_col).ne(""));
        let wanted = normalize_search(query);
        if !wanted.is_empty() {
            cond = cond.add(contains(o(key_col), &like_contains(&wanted)));
        }
        if let (OrderValueField::Model, Some(b)) = (field, brand) {
            cond = cond.add(o(order::Column::BrandKey).eq(normalize_search(b)));
        }
        let mut q = Query::select();
        q.column(key_col)
            .column(raw_col)
            .expr(Func::count(Expr::col(Asterisk)))
            .from(order::Entity)
            .cond_where(cond)
            .group_by_col(key_col)
            .group_by_col(raw_col);

        // key -> (всего, лучшее написание, его частота)
        let mut keys: HashMap<String, (u64, String, u64)> = HashMap::new();
        for row in self
            .db
            .query_all_raw(StatementBuilder::build(&q, &backend))
            .await?
        {
            let key: String = row.try_get_by_index(0)?;
            let raw: String = row.try_get_by_index(1)?;
            let count = u64::try_from(row.try_get_by_index::<i64>(2)?).unwrap_or(0);
            let e = keys.entry(key).or_insert((0, raw.clone(), 0));
            e.0 += count;
            if count > e.2 || (count == e.2 && raw < e.1) {
                e.1 = raw;
                e.2 = count;
            }
        }
        let mut list: Vec<(String, u64, String)> = keys
            .into_iter()
            .map(|(k, (n, raw, _))| (k, n, raw))
            .collect();
        list.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        Ok(list
            .into_iter()
            .take(VALUES_LIMIT)
            .map(|(_, _, raw)| raw)
            .collect())
    }
}
