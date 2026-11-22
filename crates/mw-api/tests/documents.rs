//! Документы заказа в PDF и PNG.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use base64::Engine as _;
use common::{Listen, apis, create_order, new_order};

const STICKER_58X40: &str = "label_w_mm=58&label_h_mm=40";

fn png_size(png: &[u8]) -> (u32, u32) {
    let be = |at: usize| u32::from_be_bytes(png[at..at + 4].try_into().unwrap());
    (be(16), be(20))
}

#[tokio::test]
async fn order_act_comes_as_pdf_named_after_the_order() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        let order = create_order(&t, &token, new_order("Иван", "", "A", "B")).await;
        let id = order["id"].as_i64().unwrap();
        for (kind, name) in [
            ("intake_act", "akt-priema"),
            ("issue_act", "akt-vydachi"),
            ("warranty_intake_act", "akt-priema-garantiya"),
            ("warranty_issue_act", "akt-vydachi-garantiya"),
            ("sticker", "naklejka"),
        ] {
            let (status, headers, body) = t
                .get_raw(&format!("/api/v1/orders/{id}/documents/{kind}"), &token)
                .await;
            assert_eq!(status, 200, "{}: {kind}", t.label);
            assert_eq!(headers["content-type"], "application/pdf");
            assert_eq!(
                headers["content-disposition"],
                format!("inline; filename=\"{name}-{id}.pdf\"").as_str(),
                "{}",
                t.label
            );
            assert!(body.starts_with(b"%PDF-"), "{}: {kind}", t.label);
        }
    }
}

#[tokio::test]
async fn sticker_png_has_the_label_size_at_the_raster_dpi() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        let order = create_order(&t, &token, new_order("Иван", "", "A", "B")).await;
        let id = order["id"].as_i64().unwrap();
        // The preview is rendered at 110 dpi, the barcode snaps to the 203 dpi printer.
        let r = t
            .get(
                &format!(
                    "/api/v1/orders/{id}/documents/sticker?format=png&dpi=110&printer_dpi=203&{STICKER_58X40}"
                ),
                Some(&token),
            )
            .await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        assert_eq!(r.body["dpi"], 110);
        assert!((r.body["width_mm"].as_f64().unwrap() - 58.0).abs() < 0.01);
        assert!((r.body["height_mm"].as_f64().unwrap() - 40.0).abs() < 0.01);
        let pages = r.body["pages"].as_array().unwrap();
        assert_eq!(pages.len(), 1, "{}", t.label);
        let png = base64::engine::general_purpose::STANDARD
            .decode(pages[0].as_str().unwrap())
            .unwrap();
        assert_eq!(&png[..4], b"\x89PNG", "{}", t.label);
        let (w, h) = png_size(&png);
        let px = |mm: f64| mm / 25.4 * 110.0;
        assert!(
            (f64::from(w) - px(58.0)).abs() <= 1.0 && (f64::from(h) - px(40.0)).abs() <= 1.0,
            "{}: {w}x{h}",
            t.label
        );
        // PNG by default: 110 dpi.
        let r = t
            .get(
                &format!("/api/v1/orders/{id}/documents/sticker?format=png"),
                Some(&token),
            )
            .await;
        assert_eq!(r.body["dpi"], 110, "{}", t.label);
    }
}

#[tokio::test]
async fn wrong_parameters_are_rejected_in_russian() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        let order = create_order(&t, &token, new_order("Иван", "", "A", "B")).await;
        let id = order["id"].as_i64().unwrap();
        let base = format!("/api/v1/orders/{id}/documents/sticker");
        for (query, field) in [
            ("format=png&dpi=1000", "dpi"),
            ("format=png&dpi=71", "dpi"),
            ("printer_dpi=601", "printer_dpi"),
            ("label_w_mm=14&label_h_mm=40", "label_w_mm"),
            ("label_w_mm=58&label_h_mm=201", "label_h_mm"),
            ("offset_x_mm=10.5", "offset_x_mm"),
            ("offset_y_mm=-11", "offset_y_mm"),
        ] {
            let r = t.get(&format!("{base}?{query}"), Some(&token)).await;
            assert_eq!(r.status, 422, "{}: {query}: {}", t.label, r.body);
            assert_eq!(r.body["fields"][0]["field"], field, "{}: {query}", t.label);
            assert!(!r.body["fields"][0]["message"].as_str().unwrap().is_empty());
        }
        for query in [
            "label_w_mm=58",
            "label_h_mm=40",
            "format=pdf&dpi=203",
            "format=gif",
            "dpi=abc&format=png",
            "unknown=1",
        ] {
            let r = t.get(&format!("{base}?{query}"), Some(&token)).await;
            assert_eq!(r.status, 400, "{}: {query}: {}", t.label, r.body);
            assert_eq!(r.body["code"], "bad_request", "{}: {query}", t.label);
        }
        let r = t.get(&format!("{base}?dpi=203"), Some(&token)).await;
        assert_eq!(r.status, 400, "{}: {}", t.label, r.body);
        assert!(
            r.body["message"].as_str().unwrap().contains("png"),
            "{}: {}",
            t.label,
            r.body
        );
        let r = t
            .get("/api/v1/orders/999999/documents/sticker", Some(&token))
            .await;
        assert_eq!(r.status, 404, "{}: {}", t.label, r.body);
        let r = t
            .get(
                &format!("/api/v1/orders/{id}/documents/contract"),
                Some(&token),
            )
            .await;
        assert!(
            r.status == 400 || r.status == 404,
            "{}: {}",
            t.label,
            r.status
        );
    }
}

#[tokio::test]
async fn without_an_order_the_blank_and_calibration_print_and_the_rest_are_samples() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        let (status, headers, body) = t.get_raw("/api/v1/documents/issue_act", &token).await;
        assert_eq!(status, 200, "{}", t.label);
        assert_eq!(
            headers["content-disposition"], "inline; filename=\"akt-vydachi.pdf\"",
            "{}",
            t.label
        );
        assert!(body.starts_with(b"%PDF-"), "{}", t.label);

        let r = t
            .get(
                "/api/v1/documents/calibration?format=png&label_w_mm=40&label_h_mm=30",
                Some(&token),
            )
            .await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        assert!((r.body["width_mm"].as_f64().unwrap() - 40.0).abs() < 0.01);
        assert!((r.body["height_mm"].as_f64().unwrap() - 30.0).abs() < 0.01);

        let (status, headers, _) = t
            .get_raw("/api/v1/documents/blank_intake_act", &token)
            .await;
        assert_eq!(status, 200, "{}", t.label);
        assert_eq!(
            headers["content-disposition"], "inline; filename=\"akt-priema-pustoj.pdf\"",
            "{}",
            t.label
        );
        let (_, headers, _) = t.get_raw("/api/v1/documents/calibration", &token).await;
        assert_eq!(
            headers["content-disposition"], "inline; filename=\"kalibrovka.pdf\"",
            "{}",
            t.label
        );
    }
}

/// Документы с данными клиента не оседают в кэше браузера и прокси: оба маршрута, PDF и PNG.
#[tokio::test]
async fn documents_are_never_cached() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        let order = create_order(&t, &token, new_order("Иван", "", "A", "B")).await;
        let id = order["id"].as_i64().unwrap();
        for path in [
            format!("/api/v1/orders/{id}/documents/intake_act"),
            format!("/api/v1/orders/{id}/documents/sticker?format=png"),
            "/api/v1/documents/issue_act".to_owned(),
            "/api/v1/documents/calibration?format=png".to_owned(),
        ] {
            let (status, headers, _) = t.get_raw(&path, &token).await;
            assert_eq!(status, 200, "{}: {path}", t.label);
            assert_eq!(
                headers["cache-control"], "private, no-store",
                "{}: {path}",
                t.label
            );
        }
    }
}

#[tokio::test]
async fn documents_need_a_signed_in_user() {
    for t in apis(Listen::Network).await {
        t.create_admin_user("admin").await;
        let r = t.get("/api/v1/documents/issue_act", None).await;
        assert_eq!(r.status, 401, "{}", t.label);
        let r = t.get("/api/v1/orders/1/documents/intake_act", None).await;
        assert_eq!(r.status, 401, "{}", t.label);
    }
}
