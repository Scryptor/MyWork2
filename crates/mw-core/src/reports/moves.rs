//! Отчёт о перемещениях заказов между точками обслуживания.
//!
//! Источник — записи журнала `order.moved` (`details = {"from": …, "to": …}`). Время перемещения —
//! `audit_log.at`, то есть местное время предприятия (`Core::now()`), как и границы периода.

use std::collections::{BTreeMap, HashMap};

use chrono::NaiveDateTime;
use mw_db::entity::{audit_log, service_point};
use mw_domain::permission::Permission;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};

use super::{check_period, period_bounds, point_title};
use crate::{Actor, Core, CoreError};

/// Сколько перемещений отдаётся списком самое большее.
const MAX_ITEMS: usize = 500;

/// Условия отчёта о перемещениях.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema,
)]
#[serde(deny_unknown_fields)]
pub struct MovesFilter {
    /// Первый день периода, включительно; пусто — без нижней границы.
    pub from: Option<chrono::NaiveDate>,
    /// Последний день периода, включительно; пусто — без верхней границы.
    pub to: Option<chrono::NaiveDate>,
    /// Только перемещения из этой точки или в неё.
    pub service_point_id: Option<i64>,
}

/// Маршрут: откуда и куда, и сколько раз заказы ездили так.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct MoveRoute {
    /// Точка «откуда»; пусто — «без точки».
    #[schema(required = true)]
    pub from: Option<i64>,
    /// Подпись точки «откуда».
    pub from_name: String,
    /// Точка «куда»; пусто — «без точки».
    #[schema(required = true)]
    pub to: Option<i64>,
    /// Подпись точки «куда».
    pub to_name: String,
    /// Сколько перемещений по этому маршруту.
    pub count: u64,
}

/// Одно перемещение заказа.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct MoveItem {
    /// Когда (местное время предприятия).
    pub at: NaiveDateTime,
    /// Номер заказа.
    pub order_id: i64,
    /// Кто переместил.
    pub user_name: String,
    /// Точка «откуда»; пусто — «без точки».
    #[schema(required = true)]
    pub from: Option<i64>,
    /// Подпись точки «откуда».
    pub from_name: String,
    /// Точка «куда»; пусто — «без точки».
    #[schema(required = true)]
    pub to: Option<i64>,
    /// Подпись точки «куда».
    pub to_name: String,
}

/// Отчёт о перемещениях.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct MovesReport {
    /// Маршруты: частые сверху.
    pub routes: Vec<MoveRoute>,
    /// Последние перемещения, новые сверху; не больше 500.
    pub items: Vec<MoveItem>,
    /// Сколько перемещений в отчёте всего.
    pub total: u64,
}

/// Одно число из `details`: `null` — «без точки»; `None` — поле не того вида.
fn point_field(details: &serde_json::Value, key: &str) -> Option<Option<i64>> {
    match details.get(key)? {
        serde_json::Value::Null => Some(None),
        v => v.as_i64().map(Some),
    }
}

/// Откуда и куда из `details`; `None` у повреждённой записи.
fn parse_route(row: &audit_log::Model) -> Option<(Option<i64>, Option<i64>)> {
    let route = row
        .details
        .as_deref()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
        .and_then(|v| Some((point_field(&v, "from")?, point_field(&v, "to")?)));
    if route.is_none() {
        tracing::warn!(audit_id = row.id, "повреждённый JSON в деталях журнала");
    }
    route
}

/// Подписи точек: текущие названия; архивная — с пометкой; удалённая — по номеру.
struct PointNames(HashMap<i64, String>);

impl PointNames {
    fn label(&self, id: Option<i64>) -> String {
        match id {
            None => "Без точки".to_owned(),
            Some(id) => self
                .0
                .get(&id)
                .cloned()
                .unwrap_or_else(|| format!("Точка № {id} (удалена)")),
        }
    }
}

impl Core {
    /// Перемещения заказов между точками за период; нужно право «Отчёты».
    pub async fn moves_report(
        &self,
        actor: &Actor,
        filter: &MovesFilter,
    ) -> Result<MovesReport, CoreError> {
        actor.require(Permission::ReportsView)?;
        check_period(filter.from, filter.to)?;
        let (not_before, before) = period_bounds(filter.from, filter.to);

        let mut select =
            audit_log::Entity::find().filter(audit_log::Column::Action.eq("order.moved"));
        if let Some(at) = not_before {
            select = select.filter(audit_log::Column::At.gte(at));
        }
        if let Some(at) = before {
            select = select.filter(audit_log::Column::At.lt(at));
        }
        let rows = select
            .order_by_desc(audit_log::Column::At)
            .order_by_desc(audit_log::Column::Id)
            .all(&self.db)
            .await?;
        let names = PointNames(
            service_point::Entity::find()
                .all(&self.db)
                .await?
                .into_iter()
                .map(|p| (p.id, point_title(&p.name, p.archived)))
                .collect(),
        );

        let mut counts: BTreeMap<(Option<i64>, Option<i64>), u64> = BTreeMap::new();
        let mut items = Vec::new();
        let mut total = 0u64;
        for row in &rows {
            let Some((from, to)) = parse_route(row) else {
                continue;
            };
            let Some(order_id) = row.order_id else {
                tracing::warn!(audit_id = row.id, "перемещение без заказа в журнале");
                continue;
            };
            if let Some(id) = filter.service_point_id
                && from != Some(id)
                && to != Some(id)
            {
                continue;
            }
            total += 1;
            *counts.entry((from, to)).or_default() += 1;
            if items.len() < MAX_ITEMS {
                items.push(MoveItem {
                    at: row.at,
                    order_id,
                    user_name: row.user_name.clone(),
                    from,
                    from_name: names.label(from),
                    to,
                    to_name: names.label(to),
                });
            }
        }
        let mut routes: Vec<MoveRoute> = counts
            .into_iter()
            .map(|((from, to), count)| MoveRoute {
                from,
                from_name: names.label(from),
                to,
                to_name: names.label(to),
                count,
            })
            .collect();
        // Частые сверху; при равенстве — по подписям.
        routes.sort_by(|a, b| {
            b.count
                .cmp(&a.count)
                .then_with(|| a.from_name.cmp(&b.from_name))
                .then_with(|| a.to_name.cmp(&b.to_name))
        });
        Ok(MovesReport {
            routes,
            items,
            total,
        })
    }
}
