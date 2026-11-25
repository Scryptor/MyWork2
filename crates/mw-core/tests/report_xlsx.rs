//! Excel-файл отчёта по заказам: листы, значения, права; книга читается `calamine` из памяти.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use std::io::Cursor;

use calamine::{Data, Reader, Xlsx, open_workbook_from_rs};
use chrono::NaiveDate;
use common::*;
use mw_core::{
    ClientRef, CoreError, NewClient, NewOrder, NewOrderPart, NewStockItem, OrderPatch,
    ReportFilter, ReportGroupBy, SettingsPatch,
};
use mw_domain::money::Money;
use mw_domain::permission::Permission;

fn rub(r: i64) -> Money {
    Money::from_rubles(r)
}

fn filter(brand: &str) -> ReportFilter {
    ReportFilter {
        brand: Some(brand.into()),
        ..Default::default()
    }
}

fn open(bytes: Vec<u8>) -> Xlsx<Cursor<Vec<u8>>> {
    open_workbook_from_rs(Cursor::new(bytes)).unwrap()
}

/// Число из ячейки: Excel не различает целые и дробные.
fn num(cell: &Data) -> f64 {
    match cell {
        Data::Int(i) => *i as f64,
        Data::Float(f) => *f,
        other => panic!("не число: {other:?}"),
    }
}

fn text(cell: &Data) -> String {
    match cell {
        Data::String(s) => s.clone(),
        Data::Empty => String::new(),
        other => other.to_string(),
    }
}

/// Данные как в `report_sums_issued_orders`: А (мастер, запчасти, выдан первым) и Б (без ремонта).
async fn seed(t: &TestCore, brand: &str) -> Result<(), CoreError> {
    t.core
        .update_settings(
            &admin(),
            SettingsPatch {
                issue_required: Some(vec![]),
                ..Default::default()
            },
        )
        .await?;
    let anton = make_master(&t.core, "Антон").await;
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
                request_key: "xlsx-a".into(),
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

    let b = t
        .core
        .create_order(
            &admin(),
            NewOrder {
                client: ClientRef::New(NewClient {
                    full_name: "=Клиент Б".into(),
                    phones: phones(&["+7 921 123-45-67", "+7 921 000-00-00"]),
                    ..Default::default()
                }),
                device_type: "Телефон".into(),
                model: "Модель".into(),
                brand: brand.into(),
                malfunction: "Не включается".into(),
                ..Default::default()
            },
        )
        .await?;
    let b = t
        .core
        .update_order(
            &admin(),
            b.id,
            b.version,
            OrderPatch {
                final_price: Some(Some(rub(500))),
                work_done: Some("Без ремонта".into()),
                ..Default::default()
            },
        )
        .await?;
    t.core.issue_order(&admin(), b.id, b.version).await?;
    Ok(())
}

#[tokio::test]
async fn report_workbook_has_three_sheets() -> Result<(), CoreError> {
    for t in cores().await {
        let brand = "Книга-Бренд";
        seed(&t, brand).await?;
        let file = t.core.order_report_xlsx(&admin(), &filter(brand)).await?;
        assert_eq!(
            file.name, "Отчёт по заказам за всё время.xlsx",
            "{}",
            t.label
        );
        assert_eq!(file.ascii_name, "report.xlsx");
        assert!(file.bytes.starts_with(b"PK"));
        let mut book = open(file.bytes);
        assert_eq!(
            book.sheet_names(),
            ["Итоги", "По мастерам", "Заказы"],
            "{}",
            t.label
        );

        let sums = book.worksheet_range("Итоги").unwrap();
        let rows: Vec<(String, Data)> = sums.rows().map(|r| (text(&r[0]), r[1].clone())).collect();
        let value = |name: &str| rows.iter().find(|(k, _)| k == name).unwrap().1.clone();
        let money = |name: &str| num(&value(name));
        assert_eq!(rows[0].0, "Показатель");
        assert_eq!(value("Период"), Data::String("За всё время".into()));
        assert_eq!(value("Бренд"), Data::String(brand.into()));
        assert_eq!(money("Выручка"), 1400.0, "{}", t.label);
        assert_eq!(money("Заказов"), 2.0);
        assert_eq!(money("Из них без ремонта"), 1.0);
        assert_eq!(money("Скидки"), 100.0);
        assert_eq!(money("Затраты"), 300.0);
        assert_eq!(money("Прибыль"), 1100.0);
        assert_eq!(money("Оплата мастерам"), 300.0);
        assert_eq!(value("Процент мастера"), Data::String("50 %".into()));

        let by_master = book.worksheet_range("По мастерам").unwrap();
        let head: Vec<String> = by_master.rows().next().unwrap().iter().map(text).collect();
        assert_eq!(
            head,
            [
                "Мастер",
                "Заказов",
                "Без ремонта",
                "Выручка",
                "Затраты",
                "Прибыль",
                "Оплата мастеру"
            ]
        );
        let total = by_master.rows().next_back().unwrap();
        assert_eq!(text(&total[0]), "Итого");
        assert_eq!(num(&total[1]), 2.0, "{}", t.label);
        assert_eq!(num(&total[3]), 1400.0);
        let labels: Vec<String> = by_master.rows().skip(1).map(|r| text(&r[0])).collect();
        assert_eq!(labels.len(), 3, "{labels:?}");
        assert!(labels.contains(&"Антон".to_owned()) && labels.contains(&"Без мастера".to_owned()));

        let orders = book.worksheet_range("Заказы").unwrap();
        let head: Vec<String> = orders.rows().next().unwrap().iter().map(text).collect();
        assert_eq!(head.len(), 20);
        assert_eq!(head[0], "№ заказа");
        assert_eq!(head[3], "Дней в сервисе");
        assert_eq!(head[19], "Без ремонта");
        for h in &head {
            assert!(!h.contains("Пароль") && !h.contains("ключ"), "{h}");
        }
        let col = |name: &str| head.iter().position(|h| h == name).unwrap();
        let body: Vec<_> = orders.rows().skip(1).collect();
        assert_eq!(body.len(), 2, "{}", t.label);
        assert_eq!(text(&body[0][col("Мастер")]), "Антон");
        assert_eq!(text(&body[0][col("Без ремонта")]), "");
        assert_eq!(text(&body[0][col("Выполненные работы")]), "Замена дисплея");
        assert_eq!(num(&body[0][col("Итог")]), 1000.0);
        assert_eq!(num(&body[0][col("Скидка")]), 100.0);
        assert_eq!(num(&body[0][col("Затраты")]), 300.0);
        assert_eq!(num(&body[0][col("Прибыль")]), 600.0);
        assert_eq!(num(&body[0][col("Оплата мастеру")]), 300.0);
        assert_eq!(body[1][col("Без ремонта")], Data::String("да".into()));
        // Текст «=…» остаётся текстом, а не формулой; телефон — основной.
        assert_eq!(text(&body[1][col("Клиент")]), "=Клиент Б");
        let digits: String = text(&body[1][col("Телефон")])
            .chars()
            .filter(char::is_ascii_digit)
            .collect();
        assert_eq!(digits, "79211234567");
        // Даты — настоящие даты Excel; в сервисе — 0 дней (приняты и выданы по часам теста).
        assert!(matches!(body[1][col("Принят")], Data::DateTime(_)));
        assert!(matches!(body[1][col("Выдан")], Data::DateTime(_)));
        assert_eq!(num(&body[1][col("Дней в сервисе")]), 0.0);
        assert!(num(&body[0][col("№ заказа")]) < num(&body[1][col("№ заказа")]));
    }
    Ok(())
}

#[tokio::test]
async fn period_and_group_name_the_file_and_sheets() -> Result<(), CoreError> {
    for t in cores().await {
        let f = ReportFilter {
            from: NaiveDate::from_ymd_opt(2026, 11, 1),
            to: NaiveDate::from_ymd_opt(2026, 11, 30),
            group_by: ReportGroupBy::Brand,
            ..Default::default()
        };
        let file = t.core.order_report_xlsx(&admin(), &f).await?;
        assert_eq!(
            file.name, "Отчёт по заказам 01.11.2026\u{2013}30.11.2026.xlsx",
            "{}",
            t.label
        );
        let mut book = open(file.bytes);
        assert_eq!(book.sheet_names(), ["Итоги", "По брендам", "Заказы"]);
        let sums = book.worksheet_range("Итоги").unwrap();
        let period = sums.rows().find(|r| text(&r[0]) == "Период").unwrap();
        assert_eq!(text(&period[1]), "01.11.2026\u{2013}30.11.2026");
        // Пустой отчёт: в «Заказах» только шапка, в разрезе — только «Итого».
        assert_eq!(book.worksheet_range("Заказы").unwrap().rows().count(), 1);
        assert_eq!(
            book.worksheet_range("По брендам").unwrap().rows().count(),
            2
        );
    }
    Ok(())
}

#[tokio::test]
async fn report_xlsx_needs_reports_view() -> Result<(), CoreError> {
    for t in cores().await {
        let res = t
            .core
            .order_report_xlsx(
                &actor_with(&[Permission::StockView]),
                &ReportFilter::default(),
            )
            .await;
        assert!(
            matches!(res, Err(CoreError::Forbidden(Permission::ReportsView))),
            "{}: {res:?}",
            t.label
        );
        t.core
            .order_report_xlsx(
                &actor_with(&[Permission::ReportsView]),
                &ReportFilter::default(),
            )
            .await?;
        let reversed = ReportFilter {
            from: NaiveDate::from_ymd_opt(2026, 3, 12),
            to: NaiveDate::from_ymd_opt(2026, 3, 11),
            ..Default::default()
        };
        assert_validation(
            t.core.order_report_xlsx(&admin(), &reversed).await,
            &[("to", "period_reversed")],
        );
    }
    Ok(())
}
