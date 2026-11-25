//! Отчёт по заказам по HTTP: итоги, права, неверный период, файл Excel с русским именем.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{Listen, TestApi, apis, create_order, new_order};
use mw_core::{Actor, OrderPatch, SettingsPatch};
use mw_domain::money::Money;
use mw_domain::permission::Permission;
use serde_json::json;

/// Два выданных заказа указанного бренда (выдача — по часам теста, 2026-11-11).
async fn two_issued(t: &TestApi, token: &str, brand: &str) {
    let admin = Actor::unrestricted("Тест");
    t.core
        .update_settings(
            &admin,
            SettingsPatch {
                issue_required: Some(vec![]),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    for n in 0..2 {
        let o = create_order(t, token, new_order("Иван", "", brand, "Модель")).await;
        let (id, version) = (o["id"].as_i64().unwrap(), o["version"].as_i64().unwrap());
        let o = t
            .core
            .update_order(
                &admin,
                id,
                version,
                OrderPatch {
                    final_price: Some(Some(Money::from_rubles(100 * (n + 1)))),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        t.core.issue_order(&admin, id, o.version).await.unwrap();
    }
}

/// Раскодирует `filename*=UTF-8''…` из `Content-Disposition`.
fn decoded_name(header: &str) -> String {
    let encoded = header.split("filename*=UTF-8''").nth(1).unwrap();
    let bytes = encoded.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap();
            out.push(u8::from_str_radix(hex, 16).unwrap());
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap()
}

#[tokio::test]
async fn report_endpoint_returns_totals() {
    for t in apis(Listen::Network).await {
        let token = t
            .token_with(
                "otchet",
                &[Permission::OrdersCreate, Permission::ReportsView],
            )
            .await;
        two_issued(&t, &token, "Отчёт-Бренд").await;
        let r = t
            .get(
                "/api/v1/reports/orders?brand=%D0%9E%D1%82%D1%87%D1%91%D1%82-%D0%91%D1%80%D0%B5%D0%BD%D0%B4",
                Some(&token),
            )
            .await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        assert_eq!(r.body["totals"]["orders"], json!(2), "{}", t.label);
        assert_eq!(r.body["totals"]["revenue"], json!(30000), "{}", t.label);
        assert_eq!(r.body["master_percent"], json!(50));

        // Пустые строки — «без фильтра».
        let r = t
            .get(
                "/api/v1/reports/orders?device_type=&brand=&source=&group_by=brand",
                Some(&token),
            )
            .await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        assert!(r.body["totals"]["orders"].as_u64().unwrap() >= 2);

        let plain = t.token_with("bez-prav", &[Permission::OrdersCreate]).await;
        let r = t.get("/api/v1/reports/orders", Some(&plain)).await;
        assert_eq!(r.status, 403, "{}", t.label);
        let r = t.get("/api/v1/reports/orders/xlsx", Some(&plain)).await;
        assert_eq!(r.status, 403, "{}", t.label);
        // Отчёт без права «Выгрузка» файлом не выгрузить, хотя на экране он виден.
        let r = t.get("/api/v1/reports/orders/xlsx", Some(&token)).await;
        assert_eq!(r.status, 403, "{}: {}", t.label, r.body);
        let r = t.get("/api/v1/reports/orders", None).await;
        assert_eq!(r.status, 401, "{}", t.label);

        let r = t
            .get(
                "/api/v1/reports/orders?from=2026-03-12&to=2026-03-11",
                Some(&token),
            )
            .await;
        assert_eq!(
            (r.status, &r.body["fields"][0]["code"]),
            (422, &json!("period_reversed")),
            "{}: {}",
            t.label,
            r.body
        );
    }
}

#[tokio::test]
async fn report_xlsx_has_a_russian_file_name() {
    for t in apis(Listen::Network).await {
        let token = t
            .token_with("otchet", &[Permission::ReportsView, Permission::DataExport])
            .await;
        let (status, headers, body) = t
            .get_raw(
                "/api/v1/reports/orders/xlsx?from=2026-11-01&to=2026-11-30",
                &token,
            )
            .await;
        assert_eq!(status, 200, "{}", t.label);
        assert_eq!(
            headers["content-type"],
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        );
        let disposition = headers["content-disposition"].to_str().unwrap();
        assert!(
            disposition.starts_with("attachment; filename=\"report.xlsx\"; filename*=UTF-8''"),
            "{disposition}"
        );
        assert_eq!(
            decoded_name(disposition),
            "Отчёт по заказам 01.11.2026\u{2013}30.11.2026.xlsx"
        );
        assert_eq!(headers["cache-control"], "private, no-store");
        assert_eq!(headers["content-length"], body.len().to_string().as_str());
        assert!(body.starts_with(b"PK"), "{}", t.label);
    }
}

#[tokio::test]
async fn unknown_query_fields_are_refused() {
    for t in apis(Listen::Network).await {
        let token = t.token_with("otchet", &[Permission::ReportsView]).await;
        for path in ["/api/v1/reports/orders", "/api/v1/reports/orders/xlsx"] {
            let r = t.get(&format!("{path}?brandd=x"), Some(&token)).await;
            assert_eq!(
                (r.status, &r.body["code"]),
                (400, &json!("bad_request")),
                "{}: {path}",
                t.label
            );
        }
    }
}

#[tokio::test]
async fn moves_endpoint() {
    for t in apis(Listen::Network).await {
        let token = t
            .token_with(
                "peremeshcheniya",
                &[
                    Permission::OrdersCreate,
                    Permission::OrdersEdit,
                    Permission::ReportsView,
                ],
            )
            .await;
        let admin = Actor::unrestricted("Тест");
        let a = t
            .core
            .create_service_point(&admin, "Север")
            .await
            .unwrap()
            .id;
        let b = t.core.create_service_point(&admin, "Юг").await.unwrap().id;
        let mut n = new_order("Иван", "", "Бренд", "Модель");
        n["service_point_id"] = json!(a);
        let o = create_order(&t, &token, n).await;
        t.core
            .update_order(
                &admin,
                o["id"].as_i64().unwrap(),
                o["version"].as_i64().unwrap(),
                OrderPatch {
                    service_point_id: Some(Some(b)),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let r = t.get("/api/v1/reports/moves", Some(&token)).await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        assert_eq!(r.body["total"], json!(1), "{}", t.label);
        assert_eq!(r.body["routes"][0]["from_name"], json!("Север"));
        assert_eq!(r.body["routes"][0]["to_name"], json!("Юг"));
        assert_eq!(r.body["routes"][0]["count"], json!(1));
        assert_eq!(r.body["items"][0]["order_id"], o["id"]);

        // Точка и период.
        let r = t
            .get(
                &format!("/api/v1/reports/moves?service_point_id={b}&from=2000-01-01"),
                Some(&token),
            )
            .await;
        assert_eq!(
            (r.status, &r.body["total"]),
            (200, &json!(1)),
            "{}",
            t.label
        );
        let r = t
            .get("/api/v1/reports/moves?to=2000-01-01", Some(&token))
            .await;
        assert_eq!(
            (r.status, &r.body["total"]),
            (200, &json!(0)),
            "{}",
            t.label
        );

        let r = t
            .get(
                "/api/v1/reports/moves?from=2026-03-12&to=2026-03-11",
                Some(&token),
            )
            .await;
        assert_eq!(
            (r.status, &r.body["fields"][0]["code"]),
            (422, &json!("period_reversed")),
            "{}: {}",
            t.label,
            r.body
        );
        let r = t.get("/api/v1/reports/moves?brand=x", Some(&token)).await;
        assert_eq!(r.status, 400, "{}", t.label);

        let plain = t.token_with("bez-prav", &[Permission::OrdersCreate]).await;
        let r = t.get("/api/v1/reports/moves", Some(&plain)).await;
        assert_eq!(r.status, 403, "{}", t.label);
        let r = t.get("/api/v1/reports/moves", None).await;
        assert_eq!(r.status, 401, "{}", t.label);
    }
}
