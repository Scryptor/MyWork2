//! Отчёт по выданным заказам: итоги, разрезы и шкала времени.
//!
//! В отчёт входят заказы, выданные сейчас (вид текущего статуса «выдан»), не удалённые, с датой
//! выдачи в периоде. Суммы считаются в Rust с проверкой переполнения: `SUM` на PostgreSQL отдаёт
//! `numeric`, а строк на одну выборку немного.

use std::collections::{BTreeMap, HashMap};

use chrono::{Datelike, Days, NaiveDate, NaiveDateTime};
use mw_db::entity::{client, client_phone, order, order_part, service_point, status, user};
use mw_domain::money::Money;
use mw_domain::permission::Permission;
use mw_domain::report::{is_without_repair, master_pay};
use mw_domain::search::normalize_search;
use mw_domain::status::StatusKind;
use sea_orm::sea_query::{Condition, Expr, ExprTrait, Order, Query};
use sea_orm::{ConnectionTrait, EntityTrait, StatementBuilder};
use serde::{Deserialize, Serialize};

use super::{check_period, period_bounds, point_title};
use crate::{Actor, Core, CoreError};

/// Самый длинный период, который рисуется по дням; дальше — по месяцам.
const MAX_DAY_STEPS: i64 = 62;

/// Сколько точек шкалы отдаётся самое большее (при шаге «месяц»).
const MAX_TIMELINE_POINTS: u32 = 600;

/// Подпись группы без значения в разрезе (кроме мастера и точки).
const UNSPECIFIED: &str = "Не указан";

/// Разрез отчёта.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ReportGroupBy {
    /// По мастерам.
    #[default]
    Master,
    /// По точкам обслуживания.
    ServicePoint,
    /// По типам устройств.
    DeviceType,
    /// По брендам.
    Brand,
    /// По источникам: откуда клиент узнал о сервисе.
    Source,
}

/// Шаг шкалы времени.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ReportStep {
    /// Точка — день.
    Day,
    /// Точка — месяц (первое число).
    Month,
}

/// Условия отчёта по заказам.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema,
)]
#[serde(deny_unknown_fields)]
pub struct ReportFilter {
    /// Первый день периода по дате выдачи, включительно; пусто — без нижней границы.
    pub from: Option<NaiveDate>,
    /// Последний день периода по дате выдачи, включительно; пусто — без верхней границы.
    pub to: Option<NaiveDate>,
    /// Только эта точка обслуживания.
    pub service_point_id: Option<i64>,
    /// Только этот мастер (пользователь).
    pub master_user_id: Option<i64>,
    /// Тип устройства; сравнение без учёта регистра.
    pub device_type: Option<String>,
    /// Бренд; сравнение без учёта регистра.
    pub brand: Option<String>,
    /// Источник клиента; сравнение без учёта регистра.
    pub source: Option<String>,
    /// Разрез: по чему делить итоги на группы; по умолчанию — по мастерам.
    #[serde(default)]
    pub group_by: ReportGroupBy,
}

/// Итоги по набору заказов.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema,
)]
pub struct ReportTotals {
    /// Сколько заказов.
    pub orders: u64,
    /// Из них закрыто без ремонта.
    pub without_repair: u64,
    /// Выручка: итог минус скидка.
    pub revenue: Money,
    /// Скидки.
    pub discounts: Money,
    /// Затраты: расходы и запчасти.
    pub costs: Money,
    /// Прибыль: выручка минус затраты.
    pub profit: Money,
    /// Оплата мастерам.
    pub master_pay: Money,
}

/// Группа разреза.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct OrderReportGroup {
    /// Ключ группы (для фильтра и сравнения).
    pub key: String,
    /// Подпись для человека.
    pub label: String,
    /// Итоги группы.
    pub totals: ReportTotals,
}

/// Точка шкалы времени.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct ReportPoint {
    /// День или первое число месяца.
    pub start: NaiveDate,
    /// Итоги за этот день или месяц.
    pub totals: ReportTotals,
}

/// Отчёт по выданным заказам.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct OrderReport {
    /// Итоги по всем заказам отчёта.
    pub totals: ReportTotals,
    /// Группы выбранного разреза: большая выручка сверху.
    pub groups: Vec<OrderReportGroup>,
    /// Шаг шкалы времени.
    pub step: ReportStep,
    /// Шкала времени; пустые дни и месяцы — нули.
    pub timeline: Vec<ReportPoint>,
    /// Процент оплаты мастеру, по которому посчитаны итоги.
    pub master_percent: u8,
}

/// Заказ отчёта со всем, что нужно листу «Заказы» в Excel.
#[derive(Debug, Clone)]
pub(crate) struct ReportOrderRow {
    pub id: i64,
    pub received_at: NaiveDateTime,
    /// `None` — выдан без даты выдачи (в шкалу времени не попадает).
    pub issued_at: Option<NaiveDateTime>,
    pub client_name: String,
    /// Основной телефон клиента (первый); пусто, если телефонов нет.
    pub phone: String,
    pub device_type: String,
    pub brand: String,
    pub model: String,
    pub malfunction: String,
    pub work_done: String,
    /// Имя мастера: пользователя, а у старых заказов — как записано текстом.
    pub master: String,
    pub master_user_id: Option<i64>,
    pub master_key: String,
    pub service_point_id: Option<i64>,
    /// Название точки; пусто, если точки нет.
    pub service_point: String,
    /// Точка в архиве.
    pub service_point_archived: bool,
    pub source: String,
    /// Источник без регистра и лишних пробелов.
    pub source_key: String,
    pub device_type_key: String,
    pub brand_key: String,
    /// Итог (окончательная стоимость до скидки).
    pub price: Money,
    pub discount: Money,
    /// Итог минус скидка.
    pub revenue: Money,
    /// Расходы и запчасти.
    pub costs: Money,
    pub profit: Money,
    pub master_pay: Money,
    pub without_repair: bool,
}

fn overflow() -> CoreError {
    CoreError::Internal("сумма отчёта не помещается в число".into())
}

fn add(a: Money, b: Money) -> Result<Money, CoreError> {
    a.kopecks()
        .checked_add(b.kopecks())
        .map(Money::from_kopecks)
        .ok_or_else(overflow)
}

fn sub(a: Money, b: Money) -> Result<Money, CoreError> {
    a.kopecks()
        .checked_sub(b.kopecks())
        .map(Money::from_kopecks)
        .ok_or_else(overflow)
}

impl ReportTotals {
    fn add_row(&mut self, row: &ReportOrderRow) -> Result<(), CoreError> {
        self.orders += 1;
        self.without_repair += u64::from(row.without_repair);
        self.revenue = add(self.revenue, row.revenue)?;
        self.discounts = add(self.discounts, row.discount)?;
        self.costs = add(self.costs, row.costs)?;
        self.profit = add(self.profit, row.profit)?;
        self.master_pay = add(self.master_pay, row.master_pay)?;
        Ok(())
    }
}

/// Самое частое написание на каждый ключ; при равенстве — меньшее по порядку символов.
#[derive(Default)]
struct Spellings(HashMap<String, HashMap<String, u64>>);

impl Spellings {
    fn add(&mut self, key: &str, raw: &str) {
        let raw = raw.trim();
        if !raw.is_empty() {
            *self
                .0
                .entry(key.to_owned())
                .or_default()
                .entry(raw.to_owned())
                .or_default() += 1;
        }
    }

    fn best(&self, key: &str) -> Option<String> {
        let spellings = self.0.get(key)?;
        spellings
            .iter()
            .max_by(|a, b| a.1.cmp(b.1).then_with(|| b.0.cmp(a.0)))
            .map(|(raw, _)| raw.clone())
    }
}

/// Условия, общие для заказов и запчастей: выдан сейчас, не удалён, период, точка, мастер, ключи.
fn base_condition(filter: &ReportFilter) -> Condition {
    let o = |c: order::Column| Expr::col((order::Entity, c));
    let mut cond = Condition::all()
        .add(Expr::col((status::Entity, status::Column::Kind)).eq(StatusKind::Issued.as_str()))
        .add(o(order::Column::DeletedAt).is_null());
    let (not_before, before) = period_bounds(filter.from, filter.to);
    if let Some(at) = not_before {
        cond = cond.add(o(order::Column::IssuedAt).gte(at));
    }
    if let Some(at) = before {
        cond = cond.add(o(order::Column::IssuedAt).lt(at));
    }
    if let Some(id) = filter.service_point_id {
        cond = cond.add(o(order::Column::ServicePointId).eq(id));
    }
    if let Some(id) = filter.master_user_id {
        cond = cond.add(o(order::Column::MasterUserId).eq(id));
    }
    for (value, column) in [
        (&filter.device_type, order::Column::DeviceTypeKey),
        (&filter.brand, order::Column::BrandKey),
    ] {
        let key = value.as_deref().map(normalize_search).unwrap_or_default();
        if !key.is_empty() {
            cond = cond.add(o(column).eq(key));
        }
    }
    cond
}

fn order_status_join(
    q: &mut sea_orm::sea_query::SelectStatement,
) -> &mut sea_orm::sea_query::SelectStatement {
    q.inner_join(
        status::Entity,
        Expr::col((order::Entity, order::Column::StatusId))
            .equals((status::Entity, status::Column::Id)),
    )
}

impl Core {
    /// Заказы отчёта по возрастанию даты выдачи, затем номера, с посчитанными суммами.
    pub(crate) async fn report_rows(
        &self,
        filter: &ReportFilter,
    ) -> Result<Vec<ReportOrderRow>, CoreError> {
        let backend = self.db.get_database_backend();
        let percent = self.settings().master_percent;

        let o = |c: order::Column| (order::Entity, c);
        let mut q = Query::select();
        q.columns([
            o(order::Column::Id),
            o(order::Column::ReceivedAt),
            o(order::Column::IssuedAt),
            o(order::Column::DeviceType),
            o(order::Column::Brand),
            o(order::Column::Model),
            o(order::Column::Malfunction),
            o(order::Column::WorkDone),
            o(order::Column::Master),
            o(order::Column::MasterUserId),
            o(order::Column::MasterKey),
            o(order::Column::ServicePointId),
            o(order::Column::FinalPrice),
            o(order::Column::Discount),
            o(order::Column::ExtraCosts),
            o(order::Column::DeviceTypeKey),
            o(order::Column::BrandKey),
        ])
        .columns([
            (client::Entity, client::Column::FullName),
            (client::Entity, client::Column::Source),
        ])
        .column((client_phone::Entity, client_phone::Column::Phone))
        .from(order::Entity);
        order_status_join(&mut q)
            .inner_join(
                client::Entity,
                Expr::col((order::Entity, order::Column::ClientId))
                    .equals((client::Entity, client::Column::Id)),
            )
            .left_join(
                client_phone::Entity,
                Condition::all()
                    .add(
                        Expr::col((client_phone::Entity, client_phone::Column::ClientId))
                            .equals((order::Entity, order::Column::ClientId)),
                    )
                    .add(Expr::col((client_phone::Entity, client_phone::Column::Position)).eq(0)),
            )
            .cond_where(base_condition(filter))
            .order_by((order::Entity, order::Column::IssuedAt), Order::Asc)
            .order_by((order::Entity, order::Column::Id), Order::Asc);

        let source_key = filter
            .source
            .as_deref()
            .map(normalize_search)
            .unwrap_or_default();
        let raw_rows = self
            .db
            .query_all_raw(StatementBuilder::build(&q, &backend))
            .await?;

        // Названия точек и имена мастеров-пользователей.
        let points: HashMap<i64, service_point::Model> = service_point::Entity::find()
            .all(&self.db)
            .await?
            .into_iter()
            .map(|p| (p.id, p))
            .collect();
        let users: HashMap<i64, String> = user::Entity::find()
            .all(&self.db)
            .await?
            .into_iter()
            .map(|u| (u.id, u.display_name))
            .collect();
        let parts = self.report_parts_cost(filter).await?;

        let mut rows = Vec::with_capacity(raw_rows.len());
        for r in &raw_rows {
            let source: String = r.try_get_by_index(18)?;
            if !source_key.is_empty() && normalize_search(&source) != source_key {
                continue;
            }
            let id: i64 = r.try_get_by_index(0)?;
            let issued_at: Option<NaiveDateTime> = r.try_get_by_index(2)?;
            let work_done: String = r.try_get_by_index(7)?;
            let master_text: String = r.try_get_by_index(8)?;
            let master_user_id: Option<i64> = r.try_get_by_index(9)?;
            let master_key: String = r.try_get_by_index(10)?;
            let service_point_id: Option<i64> = r.try_get_by_index(11)?;
            let price = Money::from_kopecks(r.try_get_by_index::<Option<i64>>(12)?.unwrap_or(0));
            let discount = Money::from_kopecks(r.try_get_by_index(13)?);
            let extra_costs = Money::from_kopecks(r.try_get_by_index(14)?);

            let revenue = sub(price, discount)?;
            let costs = add(
                extra_costs,
                Money::from_kopecks(parts.get(&id).copied().unwrap_or(0)),
            )?;
            let profit = sub(revenue, costs)?;
            let has_master = master_user_id.is_some() || !master_key.is_empty();
            rows.push(ReportOrderRow {
                id,
                received_at: r.try_get_by_index(1)?,
                issued_at,
                client_name: r.try_get_by_index(17)?,
                phone: r
                    .try_get_by_index::<Option<String>>(19)?
                    .unwrap_or_default(),
                device_type: r.try_get_by_index(3)?,
                brand: r.try_get_by_index(4)?,
                model: r.try_get_by_index(5)?,
                malfunction: r.try_get_by_index(6)?,
                without_repair: is_without_repair(&work_done),
                work_done,
                master: master_user_id
                    .and_then(|u| users.get(&u).cloned())
                    .unwrap_or(master_text),
                master_user_id,
                master_key,
                service_point: service_point_id
                    .and_then(|p| points.get(&p))
                    .map(|p| p.name.clone())
                    .unwrap_or_default(),
                service_point_archived: service_point_id
                    .and_then(|p| points.get(&p))
                    .is_some_and(|p| p.archived),
                service_point_id,
                source_key: normalize_search(&source),
                source,
                device_type_key: r.try_get_by_index(15)?,
                brand_key: r.try_get_by_index(16)?,
                price,
                discount,
                revenue,
                costs,
                profit,
                master_pay: if has_master {
                    master_pay(profit, percent)
                } else {
                    Money::ZERO
                },
            });
        }
        Ok(rows)
    }

    /// Σ цена × количество запчастей по заказам отчёта, в копейках.
    async fn report_parts_cost(
        &self,
        filter: &ReportFilter,
    ) -> Result<HashMap<i64, i64>, CoreError> {
        let backend = self.db.get_database_backend();
        let mut q = Query::select();
        q.columns([
            (order_part::Entity, order_part::Column::OrderId),
            (order_part::Entity, order_part::Column::Price),
            (order_part::Entity, order_part::Column::Quantity),
        ])
        .from(order_part::Entity)
        .inner_join(
            order::Entity,
            Expr::col((order_part::Entity, order_part::Column::OrderId))
                .equals((order::Entity, order::Column::Id)),
        );
        order_status_join(&mut q).cond_where(base_condition(filter));
        let mut sums: HashMap<i64, i64> = HashMap::new();
        for r in self
            .db
            .query_all_raw(StatementBuilder::build(&q, &backend))
            .await?
        {
            let order_id: i64 = r.try_get_by_index(0)?;
            let price: i64 = r.try_get_by_index(1)?;
            let quantity: i64 = r.try_get_by_index(2)?;
            let line = price.checked_mul(quantity).ok_or_else(overflow)?;
            let sum = sums.entry(order_id).or_default();
            *sum = sum.checked_add(line).ok_or_else(overflow)?;
        }
        Ok(sums)
    }

    /// Отчёт по выданным заказам; нужно право «Отчёты».
    pub async fn order_report(
        &self,
        actor: &Actor,
        filter: &ReportFilter,
    ) -> Result<OrderReport, CoreError> {
        Ok(self.report_with_rows(actor, filter).await?.0)
    }

    /// Отчёт и заказы, по которым он посчитан (одна выборка): для файла Excel.
    pub(crate) async fn report_with_rows(
        &self,
        actor: &Actor,
        filter: &ReportFilter,
    ) -> Result<(OrderReport, Vec<ReportOrderRow>), CoreError> {
        actor.require(Permission::ReportsView)?;
        check_period(filter.from, filter.to)?;
        let rows = self.report_rows(filter).await?;
        let mut totals = ReportTotals::default();
        for row in &rows {
            totals.add_row(row)?;
        }
        let (step, timeline) = timeline(filter, &rows)?;
        let report = OrderReport {
            totals,
            groups: groups(filter.group_by, &rows)?,
            step,
            timeline,
            master_percent: self.settings().master_percent,
        };
        Ok((report, rows))
    }
}

/// Ключ группы строки в этом разрезе.
fn group_key(by: ReportGroupBy, row: &ReportOrderRow) -> String {
    match by {
        ReportGroupBy::Master => match row.master_user_id {
            Some(id) => format!("user:{id}"),
            None if row.master_key.is_empty() => String::new(),
            None => format!("name:{}", row.master_key),
        },
        ReportGroupBy::ServicePoint => row
            .service_point_id
            .map(|id| id.to_string())
            .unwrap_or_default(),
        ReportGroupBy::DeviceType => row.device_type_key.clone(),
        ReportGroupBy::Brand => row.brand_key.clone(),
        ReportGroupBy::Source => row.source_key.clone(),
    }
}

pub(crate) fn groups(
    by: ReportGroupBy,
    rows: &[ReportOrderRow],
) -> Result<Vec<OrderReportGroup>, CoreError> {
    let mut spellings = Spellings::default();
    let mut totals: BTreeMap<String, ReportTotals> = BTreeMap::new();
    // Подписи точек и пользователей берутся из строк: они уже подставлены в заказы отчёта.
    let mut fixed: HashMap<String, String> = HashMap::new();
    for row in rows {
        let key = group_key(by, row);
        totals.entry(key.clone()).or_default().add_row(row)?;
        match by {
            ReportGroupBy::Master if row.master_user_id.is_some() => {
                fixed.insert(key, row.master.clone());
            }
            ReportGroupBy::Master => spellings.add(&key, &row.master),
            ReportGroupBy::ServicePoint => {
                fixed.insert(
                    key,
                    point_title(&row.service_point, row.service_point_archived),
                );
            }
            ReportGroupBy::DeviceType => spellings.add(&key, &row.device_type),
            ReportGroupBy::Brand => spellings.add(&key, &row.brand),
            ReportGroupBy::Source => spellings.add(&key, &row.source),
        }
    }
    let mut list: Vec<OrderReportGroup> = totals
        .into_iter()
        .map(|(key, totals)| {
            let label = if key.is_empty() {
                match by {
                    ReportGroupBy::Master => "Без мастера",
                    ReportGroupBy::ServicePoint => "Без точки",
                    _ => UNSPECIFIED,
                }
                .to_owned()
            } else {
                fixed
                    .get(&key)
                    .cloned()
                    .or_else(|| spellings.best(&key))
                    .unwrap_or_else(|| key.clone())
            };
            OrderReportGroup { key, label, totals }
        })
        .collect();
    // Большая выручка сверху; при равенстве группа «без значения» — последней, затем по подписи.
    list.sort_by(|a, b| {
        b.totals
            .revenue
            .cmp(&a.totals.revenue)
            .then_with(|| a.key.is_empty().cmp(&b.key.is_empty()))
            .then_with(|| a.label.cmp(&b.label))
    });
    Ok(list)
}

fn month_start(date: NaiveDate) -> NaiveDate {
    date.with_day(1).unwrap_or(date)
}

fn next_month(date: NaiveDate) -> Option<NaiveDate> {
    date.checked_add_months(chrono::Months::new(1))
}

/// Шаг и точки шкалы: по дням, если в периоде не больше 62 дней, иначе по месяцам. Без обеих
/// границ («всё время») — по месяцам от первой до последней выдачи. Нехватающая граница берётся из
/// данных; без данных шкала пуста.
fn timeline(
    filter: &ReportFilter,
    rows: &[ReportOrderRow],
) -> Result<(ReportStep, Vec<ReportPoint>), CoreError> {
    // Заказы без даты выдачи (в «всё время») на шкалу не попадают; сортировка БД ставит их
    // в начало или в конец, поэтому границы — по заказам с датой.
    let first = rows.iter().find_map(|r| r.issued_at).map(|d| d.date());
    let last = rows
        .iter()
        .rev()
        .find_map(|r| r.issued_at)
        .map(|d| d.date());
    let all_time = filter.from.is_none() && filter.to.is_none();
    let (Some(mut start), Some(mut end)) = (filter.from.or(first), filter.to.or(last)) else {
        return Ok((ReportStep::Month, Vec::new()));
    };
    let step = if !all_time && (end - start).num_days() < MAX_DAY_STEPS {
        ReportStep::Day
    } else {
        ReportStep::Month
    };

    if step == ReportStep::Month {
        // Слишком длинный период: сначала только месяцы с выдачами, потом последние месяцы.
        let months = |a: NaiveDate, b: NaiveDate| {
            (b.year() - a.year()) * 12 + b.month() as i32 - a.month() as i32 + 1
        };
        let limit = MAX_TIMELINE_POINTS as i32;
        if months(start, end) > limit
            && let (Some(first), Some(last)) = (first, last)
        {
            start = Ord::max(start, first);
            end = Ord::min(end, last);
        }
        if months(start, end) > limit
            && let Some(clipped) =
                month_start(end).checked_sub_months(chrono::Months::new(MAX_TIMELINE_POINTS - 1))
        {
            start = clipped;
        }
    }

    let mut points: Vec<ReportPoint> = Vec::new();
    let mut cursor = match step {
        ReportStep::Day => start,
        ReportStep::Month => month_start(start),
    };
    while cursor <= end {
        points.push(ReportPoint {
            start: cursor,
            totals: ReportTotals::default(),
        });
        let next = match step {
            ReportStep::Day => cursor.checked_add_days(Days::new(1)),
            ReportStep::Month => next_month(cursor),
        };
        match next {
            Some(n) => cursor = n,
            None => break,
        }
    }
    // Строки идут по возрастанию даты, точки тоже: один проход.
    let mut index = 0;
    for row in rows {
        let Some(issued) = row.issued_at else {
            continue;
        };
        let at = match step {
            ReportStep::Day => issued.date(),
            ReportStep::Month => month_start(issued.date()),
        };
        while index < points.len() && points[index].start < at {
            index += 1;
        }
        if let Some(point) = points.get_mut(index).filter(|p| p.start == at) {
            point.totals.add_row(row)?;
        }
    }
    Ok((step, points))
}
