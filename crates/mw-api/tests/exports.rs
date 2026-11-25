//! Выгрузки заказов и клиентов по HTTP: файл Excel с русским именем, права, лишние параметры.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{Listen, apis, create_order, new_order};
use mw_domain::permission::Permission;
use serde_json::json;

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
async fn orders_export_is_an_xlsx_with_a_russian_name() {
    for t in apis(Listen::Network).await {
        let token = t
            .token_with(
                "vygruzka",
                &[Permission::OrdersCreate, Permission::DataExport],
            )
            .await;
        create_order(
            &t,
            &token,
            new_order("Иван", "", "Выгрузка-Бренд", "Модель"),
        )
        .await;
        let (status, headers, body) = t
            .get_raw(
                "/api/v1/exports/orders?brand=%D0%92%D1%8B%D0%B3%D1%80%D1%83%D0%B7%D0%BA%D0%B0-%D0%91%D1%80%D0%B5%D0%BD%D0%B4&sort=id&descending=true",
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
            disposition.starts_with("attachment; filename=\"orders.xlsx\"; filename*=UTF-8''"),
            "{disposition}"
        );
        let name = decoded_name(disposition);
        assert!(
            name.starts_with("Заказы ") && name.ends_with(".xlsx"),
            "{name}"
        );
        assert_eq!(headers["cache-control"], "private, no-store");
        assert!(body.starts_with(b"PK"), "{}", t.label);
    }
}

#[tokio::test]
async fn clients_export_is_an_xlsx_with_a_russian_name() {
    for t in apis(Listen::Network).await {
        let token = t
            .token_with(
                "vygruzka-k",
                &[Permission::ClientsView, Permission::DataExport],
            )
            .await;
        let (status, headers, body) = t
            .get_raw("/api/v1/exports/clients?query=%D0%B8", &token)
            .await;
        assert_eq!(status, 200, "{}", t.label);
        let disposition = headers["content-disposition"].to_str().unwrap();
        assert!(
            disposition.starts_with("attachment; filename=\"clients.xlsx\"; filename*=UTF-8''"),
            "{disposition}"
        );
        assert!(decoded_name(disposition).starts_with("Клиенты "));
        assert!(body.starts_with(b"PK"));
    }
}

#[tokio::test]
async fn exports_refuse_paging_and_missing_rights() {
    for t in apis(Listen::Network).await {
        let token = t
            .token_with(
                "vygruzka-2",
                &[Permission::ClientsView, Permission::DataExport],
            )
            .await;
        for path in ["/api/v1/exports/orders", "/api/v1/exports/clients"] {
            for extra in ["limit=10", "offset=5"] {
                let r = t.get(&format!("{path}?{extra}"), Some(&token)).await;
                assert_eq!(
                    (r.status, &r.body["code"]),
                    (400, &json!("bad_request")),
                    "{}: {path}?{extra}",
                    t.label
                );
            }
        }
        let plain = t
            .token_with(
                "bez-vygruzki",
                &[Permission::ClientsView, Permission::OrdersCreate],
            )
            .await;
        for path in ["/api/v1/exports/orders", "/api/v1/exports/clients"] {
            let r = t.get(path, Some(&plain)).await;
            assert_eq!(r.status, 403, "{}: {path}", t.label);
            assert_eq!(t.get(path, None).await.status, 401, "{}", t.label);
        }
    }
}
