//! Шаблоны документов: список, правка, превью черновика, файл `.mwtemplate`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::Method;
use common::{Listen, apis};
use mw_domain::permission::Permission;
use serde_json::{Value, json};

const BASE: &str = "/api/v1/document-templates";

fn text_block(text: &str) -> Value {
    json!({
        "copies": "both",
        "body": {"type": "text", "text": text, "size_pt": null, "bold": false, "align": "left"}
    })
}

#[tokio::test]
async fn templates_round_trip_over_http() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        let r = t.get(BASE, Some(&token)).await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        assert_eq!(r.body.as_array().unwrap().len(), 6, "{}", t.label);

        let r = t.get(&format!("{BASE}/intake_act"), Some(&token)).await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        assert_eq!(r.body["version"], 0);
        assert_eq!(r.body["custom"], false);
        let mut template = r.body["template"].clone();
        template["blocks"]
            .as_array_mut()
            .unwrap()
            .push(text_block("Правка"));

        let put = |version: i64, template: Value| {
            let t = &t;
            let token = token.clone();
            async move {
                t.send(
                    Method::PUT,
                    &format!("{BASE}/intake_act"),
                    Some(&token),
                    Some(json!({"template": template, "version": version})),
                )
                .await
            }
        };
        let r = put(0, template.clone()).await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        assert_eq!(r.body["version"], 1);
        assert_eq!(r.body["custom"], true);

        let mut other = template.clone();
        other["blocks"]
            .as_array_mut()
            .unwrap()
            .push(text_block("Ещё"));
        let r = put(0, other).await;
        assert_eq!(r.status, 409, "{}: {}", t.label, r.body);

        let mut extra = template.clone();
        extra["blocks"][0]["body"]["surprise"] = json!(1);
        let r = put(1, extra).await;
        assert_eq!(r.status, 400, "{}: {}", t.label, r.body);

        let mut bad = template.clone();
        bad["page"]["size"] = json!("a5");
        bad["layout"] = json!("two_per_a4");
        let r = put(1, bad).await;
        assert_eq!(r.status, 422, "{}: {}", t.label, r.body);
        assert_eq!(r.body["fields"][0]["field"], "layout", "{}", t.label);
    }
}

#[tokio::test]
async fn preview_returns_pages() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        let r = t.get(&format!("{BASE}/intake_act"), Some(&token)).await;
        let template = r.body["template"].clone();
        let body = json!({"template": template});
        let r = t
            .send(
                Method::POST,
                &format!("{BASE}/intake_act/preview"),
                Some(&token),
                Some(body.clone()),
            )
            .await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        assert!(
            !r.body["pages"].as_array().unwrap().is_empty(),
            "{}",
            t.label
        );
        assert!((r.body["width_mm"].as_f64().unwrap() - 210.0).abs() < 0.01);

        let plain = t.token_with("plain", &[Permission::OrdersCreate]).await;
        let r = t
            .send(
                Method::POST,
                &format!("{BASE}/intake_act/preview"),
                Some(&plain),
                Some(body),
            )
            .await;
        assert_eq!(r.status, 403, "{}: {}", t.label, r.body);
    }
}

#[tokio::test]
async fn file_export_and_import() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        let (status, headers, body) = t.get_raw(&format!("{BASE}/intake_act/file"), &token).await;
        assert_eq!(status, 200, "{}", t.label);
        assert_eq!(headers["content-type"], "application/json");
        let disposition = headers["content-disposition"].to_str().unwrap();
        assert!(disposition.starts_with("attachment"), "{disposition}");
        assert!(
            disposition.contains("akt-priema.mwtemplate"),
            "{disposition}"
        );
        let file: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(file["format"], "mwtemplate");
        assert_eq!(file["format_version"], 2);

        let r = t
            .send(
                Method::POST,
                &format!("{BASE}/issue_act/file"),
                Some(&token),
                Some(file),
            )
            .await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        assert_eq!(r.body["kind"], "issue_act");

        let r = t
            .send(
                Method::POST,
                &format!("{BASE}/issue_act/file"),
                Some(&token),
                Some(json!({"format": "x"})),
            )
            .await;
        assert_eq!(r.status, 422, "{}: {}", t.label, r.body);
        assert_eq!(r.body["fields"][0]["field"], "format", "{}", t.label);
    }
}

#[tokio::test]
async fn import_into_wrong_kind_is_422() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        let (_, _, body) = t.get_raw(&format!("{BASE}/intake_act/file"), &token).await;
        let file: Value = serde_json::from_slice(&body).unwrap();
        let r = t
            .send(
                Method::POST,
                &format!("{BASE}/sticker/file"),
                Some(&token),
                Some(file),
            )
            .await;
        assert_eq!(r.status, 422, "{}: {}", t.label, r.body);
        assert_eq!(r.body["fields"][0]["field"], "layout", "{}", t.label);
        assert_eq!(r.body["fields"][0]["code"], "not_allowed", "{}", t.label);
    }
}

#[tokio::test]
async fn calibration_is_422() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        let r = t.get(&format!("{BASE}/calibration"), Some(&token)).await;
        assert_eq!(r.status, 422, "{}: {}", t.label, r.body);
        assert_eq!(r.body["fields"][0]["field"], "kind", "{}", t.label);
        assert_eq!(r.body["fields"][0]["code"], "not_editable", "{}", t.label);
    }
}

#[tokio::test]
async fn fields_listed() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        let r = t.get("/api/v1/document-fields", Some(&token)).await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        assert_eq!(r.body.as_array().unwrap().len(), 33, "{}", t.label);
    }
}

#[tokio::test]
async fn printing_uses_saved_template() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        let r = t.get(&format!("{BASE}/intake_act"), Some(&token)).await;
        let mut template = r.body["template"].clone();
        template["blocks"]
            .as_array_mut()
            .unwrap()
            .push(text_block("Проверка API"));
        let r = t
            .send(
                Method::PUT,
                &format!("{BASE}/intake_act"),
                Some(&token),
                Some(json!({"template": template, "version": 0})),
            )
            .await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        let r = t
            .get("/api/v1/documents/intake_act?format=png", Some(&token))
            .await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        assert!(
            !r.body["pages"].as_array().unwrap().is_empty(),
            "{}",
            t.label
        );
    }
}
