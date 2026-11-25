//! Отчёт по выданным заказам: какие заказы входят, формулы, границы периода, разрезы, шкала времени.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use chrono::{NaiveDate, NaiveDateTime};
use common::*;
use mw_core::{
    ClientRef, CoreError, NewClient, NewOrder, NewOrderPart, NewStockItem, OrderPatch,
    ReportFilter, ReportGroupBy, ReportStep, ReportTotals, ServicePointPatch, SettingsPatch,
};
use mw_db::entity::order;
use mw_domain::money::Money;
use mw_domain::permission::Permission;
use mw_domain::search::normalize_search;
use mw_domain::status::StatusKind;
use sea_orm::{ActiveModelTrait, Set};

fn day(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> NaiveDateTime {
    day(y, m, d).and_hms_opt(h, min, 0).unwrap()
}

fn rub(r: i64) -> Money {
    Money::from_rubles(r)
}

/// Заказ для отчёта; что не названо — пусто или ноль.
#[derive(Default)]
struct Spec<'a> {
    brand: &'a str,
    device_type: &'a str,
    source: &'a str,
    master_user_id: Option<i64>,
    service_point_id: Option<i64>,
    work_done: &'a str,
    price: i64,
    discount: i64,
    extra_costs: i64,
    /// Не выдавать.
    keep_open: bool,
    /// День и время выдачи вместо «сейчас» часов теста.
    issued_at: Option<NaiveDateTime>,
}

/// Снимает требования к выдаче, чтобы выдавать заказы без мастера и без работ.
async fn relax_issue(t: &TestCore) {
    t.core
        .update_settings(
            &admin(),
            SettingsPatch {
                issue_required: Some(vec![]),
                ..Default::default()
            },
        )
        .await
        .unwrap();
}

/// Создаёт заказ (у каждого свой клиент), при необходимости выдаёт; возвращает номер и версию.
async fn make(t: &TestCore, s: Spec<'_>) -> (i64, i64) {
    let o = t
        .core
        .create_order(
            &admin(),
            NewOrder {
                client: ClientRef::New(NewClient {
                    full_name: "Клиент".into(),
                    phones: phones(&["+7 921 123-45-67"]),
                    source: s.source.into(),
                    ..Default::default()
                }),
                device_type: if s.device_type.is_empty() {
                    "Телефон".into()
                } else {
                    s.device_type.into()
                },
                model: "Модель".into(),
                brand: s.brand.into(),
                master_user_id: s.master_user_id,
                service_point_id: s.service_point_id,
                malfunction: "Не включается".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let mut o = t
        .core
        .update_order(
            &admin(),
            o.id,
            o.version,
            OrderPatch {
                work_done: Some(s.work_done.into()),
                final_price: Some(Some(rub(s.price))),
                discount: Some(rub(s.discount)),
                extra_costs: Some(rub(s.extra_costs)),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    if !s.keep_open {
        o = t.core.issue_order(&admin(), o.id, o.version).await.unwrap();
        if let Some(when) = s.issued_at {
            o = t
                .core
                .update_order(
                    &admin(),
                    o.id,
                    o.version,
                    OrderPatch {
                        issued_at: Some(Some(when)),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
        }
    }
    (o.id, o.version)
}

fn brand_filter(brand: &str) -> ReportFilter {
    ReportFilter {
        brand: Some(brand.into()),
        ..Default::default()
    }
}

#[tokio::test]
async fn report_sums_issued_orders() -> Result<(), CoreError> {
    for t in cores().await {
        relax_issue(&t).await;
        let anton = make_master(&t.core, "Антон").await;
        let brand = "Сумма-Бренд";
        let item = t
            .core
            .create_stock_item(
                &admin(),
                NewStockItem {
                    name: "Дисплей".into(),
                    quantity: 10,
                    price: rub(50),
                    ..Default::default()
                },
            )
            .await?;
        // А: запчасти ставятся до выдачи.
        let a = t
            .core
            .create_order(
                &admin(),
                NewOrder {
                    brand: brand.into(),
                    master_user_id: Some(anton),
                    ..sample_new_order()
                },
            )
            .await?;
        t.core
            .add_order_part(
                &admin(),
                a.id,
                NewOrderPart {
                    stock_item_id: item.id,
                    quantity: 2,
                    request_key: "report-a".into(),
                },
            )
            .await?;
        let a = t.core.get_order(&admin(), a.id).await?;
        let a = t
            .core
            .update_order(
                &admin(),
                a.id,
                a.version,
                OrderPatch {
                    final_price: Some(Some(rub(1000))),
                    discount: Some(rub(100)),
                    extra_costs: Some(rub(200)),
                    work_done: Some("Замена дисплея".into()),
                    ..Default::default()
                },
            )
            .await?;
        t.core.issue_order(&admin(), a.id, a.version).await?;
        // Б: без ремонта и без мастера.
        make(
            &t,
            Spec {
                brand,
                work_done: "Без ремонта",
                price: 500,
                ..Default::default()
            },
        )
        .await;
        // В: не выдан.
        make(
            &t,
            Spec {
                brand,
                price: 700,
                keep_open: true,
                ..Default::default()
            },
        )
        .await;

        let report = t.core.order_report(&admin(), &brand_filter(brand)).await?;
        assert_eq!(
            report.totals,
            ReportTotals {
                orders: 2,
                without_repair: 1,
                revenue: rub(1400),
                discounts: rub(100),
                costs: rub(300),
                profit: rub(1100),
                master_pay: rub(300),
            },
            "{}",
            t.label
        );
        assert_eq!(report.master_percent, 50);
        assert_eq!(
            report.groups.iter().map(|g| g.totals.orders).sum::<u64>(),
            2
        );
    }
    Ok(())
}

#[tokio::test]
async fn reopened_and_deleted_orders_leave_the_report() -> Result<(), CoreError> {
    for t in cores().await {
        relax_issue(&t).await;
        let brand = "Возврат-Бренд";
        let spec = || Spec {
            brand,
            price: 100,
            ..Default::default()
        };
        let (reopened, v1) = make(&t, spec()).await;
        let (deleted, v2) = make(&t, spec()).await;
        make(&t, spec()).await;
        let total = |t: &TestCore| {
            let core = t.core.clone();
            async move {
                core.order_report(&admin(), &brand_filter(brand))
                    .await
                    .unwrap()
                    .totals
                    .orders
            }
        };
        assert_eq!(total(&t).await, 3, "{}", t.label);

        let open = status_id_by_kind(&t.core, StatusKind::Open).await;
        t.core.change_status(&admin(), reopened, v1, open).await?;
        assert_eq!(total(&t).await, 2, "{}", t.label);

        t.core.delete_order(&admin(), deleted, v2).await?;
        assert_eq!(total(&t).await, 1, "{}", t.label);
    }
    Ok(())
}

#[tokio::test]
async fn period_bounds_are_inclusive_days() -> Result<(), CoreError> {
    for t in cores().await {
        relax_issue(&t).await;
        let brand = "Период-Бренд";
        for when in [at(2026, 3, 10, 23, 59), at(2026, 3, 11, 0, 0)] {
            make(
                &t,
                Spec {
                    brand,
                    price: 100,
                    issued_at: Some(when),
                    ..Default::default()
                },
            )
            .await;
        }
        let count = |from, to| {
            let core = t.core.clone();
            async move {
                core.order_report(
                    &admin(),
                    &ReportFilter {
                        from,
                        to,
                        ..brand_filter(brand)
                    },
                )
                .await
                .unwrap()
                .totals
                .orders
            }
        };
        let (d10, d11) = (Some(day(2026, 3, 10)), Some(day(2026, 3, 11)));
        assert_eq!(count(d10, d10).await, 1, "{}", t.label);
        assert_eq!(count(d11, None).await, 1);
        assert_eq!(count(None, None).await, 2);
        assert_eq!(count(None, d10).await, 1);
        assert_validation(
            t.core
                .order_report(
                    &admin(),
                    &ReportFilter {
                        from: Some(day(2026, 3, 12)),
                        to: d11,
                        ..Default::default()
                    },
                )
                .await,
            &[("to", "period_reversed")],
        );
    }
    Ok(())
}

#[tokio::test]
async fn sources_group_without_case() -> Result<(), CoreError> {
    for t in cores().await {
        relax_issue(&t).await;
        let brand = "Источник-Бренд";
        for source in ["Авито", "Авито", "авито ", "АВИТО", "Сарафан", ""]
        {
            make(
                &t,
                Spec {
                    brand,
                    source,
                    price: 100,
                    ..Default::default()
                },
            )
            .await;
        }
        let report = t
            .core
            .order_report(
                &admin(),
                &ReportFilter {
                    group_by: ReportGroupBy::Source,
                    ..brand_filter(brand)
                },
            )
            .await?;
        let got: Vec<(&str, u64)> = report
            .groups
            .iter()
            .map(|g| (g.label.as_str(), g.totals.orders))
            .collect();
        assert_eq!(
            got,
            [("Авито", 4), ("Сарафан", 1), ("Не указан", 1)],
            "{}",
            t.label
        );
        assert_eq!(report.groups[0].key, "авито");
        assert_eq!(report.groups[2].key, "");

        let filtered = t
            .core
            .order_report(
                &admin(),
                &ReportFilter {
                    source: Some(" АВИТО ".into()),
                    ..brand_filter(brand)
                },
            )
            .await?;
        assert_eq!(filtered.totals.orders, 4);
    }
    Ok(())
}

/// Меняет мастера заказа текстом, без пользователя (как у старых заказов).
async fn set_master_text(t: &TestCore, order_id: i64, text: &str) {
    order::ActiveModel {
        id: Set(order_id),
        master: Set(text.to_owned()),
        master_key: Set(normalize_search(text)),
        master_user_id: Set(None),
        ..Default::default()
    }
    .update(t.core.db())
    .await
    .unwrap();
}

/// Стирает тип устройства в обход `Core` (как у старых заказов без типа).
async fn blank_device_type(t: &TestCore, order_id: i64) {
    order::ActiveModel {
        id: Set(order_id),
        device_type: Set(String::new()),
        device_type_key: Set(String::new()),
        ..Default::default()
    }
    .update(t.core.db())
    .await
    .unwrap();
}

#[tokio::test]
async fn groups_by_master_point_type_and_brand() -> Result<(), CoreError> {
    for t in cores().await {
        relax_issue(&t).await;
        let anton = make_master(&t.core, "Антон").await;
        let point_a = t.core.create_service_point(&admin(), "Точка А").await?.id;
        let point_b = t.core.create_service_point(&admin(), "Точка Б").await?.id;
        // Выручка по убыванию: первый заказ — самый дорогой.
        let (first, _) = make(
            &t,
            Spec {
                brand: "Zeta",
                device_type: "",
                master_user_id: Some(anton),
                service_point_id: Some(point_b),
                price: 3000,
                ..Default::default()
            },
        )
        .await;
        let (second, _) = make(
            &t,
            Spec {
                brand: "zeta",
                device_type: "Ноутбук",
                master_user_id: Some(anton),
                service_point_id: Some(point_a),
                price: 2000,
                ..Default::default()
            },
        )
        .await;
        make(
            &t,
            Spec {
                brand: "Alpha",
                device_type: "ноутбук",
                price: 1000,
                ..Default::default()
            },
        )
        .await;
        set_master_text(&t, second, "Иван Петров").await;
        blank_device_type(&t, first).await;
        t.core
            .update_service_point(
                &admin(),
                point_b,
                ServicePointPatch {
                    archived: Some(true),
                    ..Default::default()
                },
            )
            .await?;

        let groups = |group_by| {
            let core = t.core.clone();
            async move {
                core.order_report(
                    &admin(),
                    &ReportFilter {
                        group_by,
                        ..Default::default()
                    },
                )
                .await
                .unwrap()
                .groups
                .into_iter()
                .map(|g| (g.key, g.label, g.totals.revenue))
                .collect::<Vec<_>>()
            }
        };
        let s = |k: &str, l: &str, r: i64| (k.to_owned(), l.to_owned(), rub(r));

        assert_eq!(
            groups(ReportGroupBy::Master).await,
            [
                s(&format!("user:{anton}"), "Антон", 3000),
                s("name:иван петров", "Иван Петров", 2000),
                s("", "Без мастера", 1000),
            ],
            "{}",
            t.label
        );
        assert_eq!(
            groups(ReportGroupBy::ServicePoint).await,
            [
                s(&point_b.to_string(), "Точка Б (в архиве)", 3000),
                s(&point_a.to_string(), "Точка А", 2000),
                s("", "Без точки", 1000),
            ]
        );
        // Тип: «ноутбук» и «Ноутбук» — один ключ, написание с одинаковой частотой — меньшее.
        assert_eq!(
            groups(ReportGroupBy::DeviceType).await,
            [s("ноутбук", "Ноутбук", 3000), s("", "Не указан", 3000),]
        );
        assert_eq!(
            groups(ReportGroupBy::Brand).await,
            [s("zeta", "Zeta", 5000), s("alpha", "Alpha", 1000)]
        );
    }
    Ok(())
}

#[tokio::test]
async fn timeline_steps_by_day_or_month() -> Result<(), CoreError> {
    for t in cores().await {
        relax_issue(&t).await;
        let brand = "Шкала-Бренд";
        for when in [at(2026, 1, 15, 10, 0), at(2026, 3, 10, 10, 0)] {
            make(
                &t,
                Spec {
                    brand,
                    price: 100,
                    issued_at: Some(when),
                    ..Default::default()
                },
            )
            .await;
        }
        let report = |from, to| {
            let core = t.core.clone();
            async move {
                core.order_report(
                    &admin(),
                    &ReportFilter {
                        from,
                        to,
                        ..brand_filter(brand)
                    },
                )
                .await
                .unwrap()
            }
        };

        let march = report(Some(day(2026, 3, 1)), Some(day(2026, 3, 31))).await;
        assert_eq!(march.step, ReportStep::Day, "{}", t.label);
        assert_eq!(march.timeline.len(), 31);
        assert_eq!(march.timeline[0].start, day(2026, 3, 1));
        assert_eq!(march.timeline[0].totals, ReportTotals::default());
        assert_eq!(march.timeline[9].start, day(2026, 3, 10));
        assert_eq!(march.timeline[9].totals.orders, 1);
        assert_eq!(march.timeline[9].totals.revenue, rub(100));

        let year = report(Some(day(2026, 1, 1)), Some(day(2026, 12, 31))).await;
        assert_eq!(year.step, ReportStep::Month);
        assert_eq!(year.timeline.len(), 12);
        assert!(
            year.timeline
                .iter()
                .enumerate()
                .all(|(i, p)| { p.start == day(2026, u32::try_from(i).unwrap() + 1, 1) })
        );
        assert_eq!(year.timeline[0].totals.orders, 1);
        assert_eq!(year.timeline[2].totals.orders, 1);

        let all = report(None, None).await;
        assert_eq!(all.step, ReportStep::Month);
        assert_eq!(
            all.timeline
                .iter()
                .map(|p| (p.start, p.totals.orders))
                .collect::<Vec<_>>(),
            [
                (day(2026, 1, 1), 1),
                (day(2026, 2, 1), 0),
                (day(2026, 3, 1), 1)
            ]
        );

        // Граница задана с одной стороны: другая берётся из данных; до 62 дней — по дням.
        let tail = report(Some(day(2026, 3, 1)), None).await;
        assert_eq!(tail.step, ReportStep::Day);
        assert_eq!(tail.timeline.len(), 10);

        let none = t
            .core
            .order_report(&admin(), &brand_filter("Нет такого"))
            .await?;
        assert!(none.timeline.is_empty());
        assert_eq!(none.totals, ReportTotals::default());
    }
    Ok(())
}

#[tokio::test]
async fn report_needs_reports_view() -> Result<(), CoreError> {
    for t in cores().await {
        let res = t
            .core
            .order_report(
                &actor_with(&[Permission::OrdersEdit]),
                &ReportFilter::default(),
            )
            .await;
        assert!(
            matches!(res, Err(CoreError::Forbidden(Permission::ReportsView))),
            "{}: {res:?}",
            t.label
        );
        t.core
            .order_report(
                &actor_with(&[Permission::ReportsView]),
                &ReportFilter::default(),
            )
            .await?;
    }
    Ok(())
}
