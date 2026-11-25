//! Отчёт о перемещениях заказов между точками: маршруты, подписи точек, период, права.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use chrono::NaiveDate;
use common::*;
use mw_core::{CoreError, MovesFilter, OrderPatch, ServicePointPatch};
use mw_db::entity::audit_log;
use mw_domain::permission::Permission;
use sea_orm::{ActiveModelTrait, NotSet, Set};

fn day(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

fn move_to(point: Option<i64>) -> OrderPatch {
    OrderPatch {
        service_point_id: Some(point),
        ..Default::default()
    }
}

/// Перемещает заказ и возвращает его новую версию.
async fn relocate(t: &TestCore, id: i64, version: i64, to: Option<i64>) -> i64 {
    t.core
        .update_order(&admin(), id, version, move_to(to))
        .await
        .unwrap()
        .version
}

/// Запись журнала о перемещении в обход `Core`.
async fn raw_move(t: &TestCore, order_id: i64, details: Option<&str>) {
    audit_log::ActiveModel {
        id: NotSet,
        at: Set(day(2026, 11, 11).and_hms_opt(9, 30, 0).unwrap()),
        user_id: Set(None),
        user_name: Set("Система".into()),
        action: Set("order.moved".into()),
        order_id: Set(Some(order_id)),
        client_id: Set(None),
        summary: Set(String::new()),
        details: Set(details.map(str::to_owned)),
        stock_item_id: Set(None),
    }
    .insert(t.core.db())
    .await
    .unwrap();
}

#[tokio::test]
async fn moves_report_counts_routes() -> Result<(), CoreError> {
    for t in cores().await {
        let a = t.core.create_service_point(&admin(), "А").await?.id;
        let b = t.core.create_service_point(&admin(), "Б").await?.id;
        let c = t.core.create_service_point(&admin(), "В").await?.id;
        let one = create_order_with(&t, |n| n.service_point_id = Some(a)).await;
        let two = create_order_with(&t, |n| n.service_point_id = Some(a)).await;
        let v = relocate(&t, one.id, one.version, Some(b)).await;
        relocate(&t, one.id, v, Some(c)).await;
        relocate(&t, two.id, two.version, Some(b)).await;

        let report = t
            .core
            .moves_report(&admin(), &MovesFilter::default())
            .await?;
        let routes: Vec<_> = report
            .routes
            .iter()
            .map(|r| {
                (
                    r.from,
                    r.from_name.as_str(),
                    r.to,
                    r.to_name.as_str(),
                    r.count,
                )
            })
            .collect();
        assert_eq!(
            routes,
            vec![
                (Some(a), "А", Some(b), "Б", 2),
                (Some(b), "Б", Some(c), "В", 1)
            ],
            "{}",
            t.label
        );
        assert_eq!(report.total, 3, "{}", t.label);
        // Новые сверху.
        let items: Vec<_> = report
            .items
            .iter()
            .map(|i| (i.order_id, i.from, i.to, i.user_name.as_str()))
            .collect();
        assert_eq!(
            items,
            vec![
                (two.id, Some(a), Some(b), "Тест"),
                (one.id, Some(b), Some(c), "Тест"),
                (one.id, Some(a), Some(b), "Тест"),
            ],
            "{}",
            t.label
        );

        // Точка — «откуда» или «куда».
        let only_c = t
            .core
            .moves_report(
                &admin(),
                &MovesFilter {
                    service_point_id: Some(c),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(only_c.total, 1, "{}", t.label);
        assert_eq!(only_c.routes.len(), 1, "{}", t.label);
        assert_eq!(
            (
                only_c.routes[0].from,
                only_c.routes[0].to,
                only_c.routes[0].count
            ),
            (Some(b), Some(c), 1)
        );
        let only_b = t
            .core
            .moves_report(
                &admin(),
                &MovesFilter {
                    service_point_id: Some(b),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(only_b.total, 3, "{}", t.label);
    }
    Ok(())
}

#[tokio::test]
async fn moves_report_names_archived_deleted_and_missing_points() -> Result<(), CoreError> {
    for t in cores().await {
        let a = t.core.create_service_point(&admin(), "А").await?.id;
        let o = create_order_with(&t, |n| n.service_point_id = None).await;
        assert_eq!(o.service_point_id, None, "{}", t.label);
        relocate(&t, o.id, o.version, Some(a)).await;
        t.core
            .update_service_point(
                &admin(),
                a,
                ServicePointPatch {
                    archived: Some(true),
                    ..Default::default()
                },
            )
            .await?;
        // Точки с таким номером нет; запись с повреждёнными подробностями не считается.
        let gone = 99_999;
        raw_move(&t, o.id, Some(&format!(r#"{{"from":{a},"to":{gone}}}"#))).await;
        raw_move(&t, o.id, Some("не json")).await;
        raw_move(&t, o.id, Some(r#"{"from":"x","to":null}"#)).await;
        raw_move(&t, o.id, None).await;

        let report = t
            .core
            .moves_report(&admin(), &MovesFilter::default())
            .await?;
        assert_eq!(report.total, 2, "{}", t.label);
        let mut names: Vec<_> = report
            .routes
            .iter()
            .map(|r| (r.from_name.clone(), r.to_name.clone()))
            .collect();
        names.sort();
        assert_eq!(
            names,
            vec![
                (
                    "А (в архиве)".to_owned(),
                    "Точка № 99999 (удалена)".to_owned()
                ),
                ("Без точки".to_owned(), "А (в архиве)".to_owned()),
            ],
            "{}",
            t.label
        );
        let item = report.items.iter().find(|i| i.from.is_none()).unwrap();
        assert_eq!(item.from_name, "Без точки", "{}", t.label);
    }
    Ok(())
}

#[tokio::test]
async fn moves_report_period_and_item_limit() -> Result<(), CoreError> {
    for t in cores().await {
        let a = t.core.create_service_point(&admin(), "А").await?.id;
        let b = t.core.create_service_point(&admin(), "Б").await?.id;
        let o = create_order_with(&t, |n| n.service_point_id = Some(a)).await;
        relocate(&t, o.id, o.version, Some(b)).await;
        let total = |from, to| {
            let core = t.core.clone();
            async move {
                core.moves_report(
                    &admin(),
                    &MovesFilter {
                        from,
                        to,
                        service_point_id: None,
                    },
                )
                .await
                .unwrap()
                .total
            }
        };
        // Перемещение сделано 2026-11-11 09:00 по часам теста.
        assert_eq!(
            total(Some(day(2026, 11, 11)), Some(day(2026, 11, 11))).await,
            1
        );
        assert_eq!(total(None, Some(day(2026, 11, 10))).await, 0);
        assert_eq!(total(Some(day(2026, 11, 12)), None).await, 0);
        assert_eq!(total(Some(day(2026, 11, 11)), None).await, 1);
        assert_eq!(total(None, Some(day(9999, 12, 31))).await, 1);

        // Больше 500: показаны 500 новых, всего — все.
        for _ in 0..500 {
            raw_move(&t, o.id, Some(&format!(r#"{{"from":{a},"to":{b}}}"#))).await;
        }
        let report = t
            .core
            .moves_report(&admin(), &MovesFilter::default())
            .await?;
        assert_eq!(
            (report.total, report.items.len()),
            (501, 500),
            "{}",
            t.label
        );
        assert_eq!(report.routes.iter().map(|r| r.count).sum::<u64>(), 501);
    }
    Ok(())
}

#[tokio::test]
async fn moves_report_needs_reports_view() {
    for t in cores().await {
        let res = t
            .core
            .moves_report(
                &actor_with(&[Permission::OrdersEdit]),
                &MovesFilter::default(),
            )
            .await;
        assert!(
            matches!(res, Err(CoreError::Forbidden(Permission::ReportsView))),
            "{}: {res:?}",
            t.label
        );
    }
}

#[tokio::test]
async fn moves_period_reversed() {
    for t in cores().await {
        let res = t
            .core
            .moves_report(
                &admin(),
                &MovesFilter {
                    from: Some(day(2026, 3, 12)),
                    to: Some(day(2026, 3, 11)),
                    service_point_id: None,
                },
            )
            .await;
        assert_validation(res, &[("to", "period_reversed")]);
    }
}
