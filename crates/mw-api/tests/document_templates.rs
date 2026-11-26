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
        assert_eq!(r.body.as_array().unwrap().len(), 7, "{}", t.label);

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
        assert_eq!(
            r.headers["cache-control"], "private, no-store",
            "{}",
            t.label
        );
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

/// Ошибка своего Typst-кода в превью: 422 с полем блока и строкой ошибки.
#[tokio::test]
async fn preview_names_the_code_block_and_line_of_an_error() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        let r = t.get(&format!("{BASE}/intake_act"), Some(&token)).await;
        let mut template = r.body["template"].clone();
        template["blocks"].as_array_mut().unwrap().insert(
            1,
            json!({"copies": "both", "body": {"type": "typst_code", "code": "#nope"}}),
        );
        let r = t
            .send(
                Method::POST,
                &format!("{BASE}/intake_act/preview"),
                Some(&token),
                Some(json!({"template": template})),
            )
            .await;
        assert_eq!(r.status, 422, "{}: {}", t.label, r.body);
        assert_eq!(r.body["code"], "validation", "{}", t.label);
        assert_eq!(
            r.body["message"], "Ошибка в своём Typst-коде",
            "{}",
            t.label
        );
        assert_eq!(r.body["fields"][0]["field"], "blocks.1.code", "{}", t.label);
        assert_eq!(r.body["fields"][0]["code"], "typst_error", "{}", t.label);
        assert_eq!(r.body["diagnostics"][0]["place"]["line"], 1, "{}", t.label);
        assert_eq!(
            r.body["diagnostics"][0]["place"]["block"],
            json!([1]),
            "{}",
            t.label
        );
    }
}

#[tokio::test]
async fn file_export_and_import() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        let (status, headers, body) = t.get_raw(&format!("{BASE}/intake_act/file"), &token).await;
        assert_eq!(status, 200, "{}", t.label);
        assert_eq!(headers["content-type"], "application/json");
        // Своего `Cache-Control` у файла шаблона нет: остаётся общий `no-store`.
        assert_eq!(headers["cache-control"], "no-store", "{}", t.label);
        let disposition = headers["content-disposition"].to_str().unwrap();
        assert!(disposition.starts_with("attachment"), "{disposition}");
        assert!(
            disposition.contains("akt-priema.mwtemplate"),
            "{disposition}"
        );
        let file: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(file["format"], "mwtemplate");
        assert_eq!(file["format_version"], 5);

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
        assert_eq!(r.body.as_array().unwrap().len(), 37 + 4, "{}", t.label);
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

#[tokio::test]
async fn legacy_template_text_is_served_or_404() {
    use mw_db::entity::{legacy_import, legacy_row};
    use sea_orm::{ActiveModelTrait, ActiveValue::NotSet, Set};

    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        // Импорта нет: 404 и у вида с файлом.
        let r = t
            .get(&format!("{BASE}/intake_act/legacy"), Some(&token))
            .await;
        assert_eq!(r.status, 404, "{}: {}", t.label, r.body);

        let import = legacy_import::ActiveModel {
            id: NotSet,
            imported_at: Set(chrono::NaiveDate::from_ymd_opt(2026, 11, 1)
                .unwrap()
                .and_hms_opt(10, 0, 0)
                .unwrap()),
            source_path: Set("fixture".to_owned()),
            mode: Set("into_empty".to_owned()),
            report: Set("{}".to_owned()),
        }
        .insert(t.core.db())
        .await
        .unwrap();
        legacy_row::ActiveModel {
            id: NotSet,
            import_id: Set(import.id),
            source: Set("file".to_owned()),
            source_key: Set("Shablon_Act_Priema.htm".to_owned()),
            target_table: Set(None),
            target_id: Set(None),
            raw: Set(
                json!({"encoding": "windows-1251", "text": "<p>FIOPRINT<o:p></o:p></p>"})
                    .to_string(),
            ),
        }
        .insert(t.core.db())
        .await
        .unwrap();

        let r = t
            .get(&format!("{BASE}/intake_act/legacy"), Some(&token))
            .await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        assert_eq!(r.body["file"], "Shablon_Act_Priema.htm", "{}", t.label);
        assert_eq!(r.body["text"], "{клиент.фио}", "{}", t.label);
        for kind in ["sticker", "issue_act"] {
            let r = t.get(&format!("{BASE}/{kind}/legacy"), Some(&token)).await;
            assert_eq!(r.status, 404, "{} {kind}: {}", t.label, r.body);
        }
        let r = t.get(&format!("{BASE}/intake_act/legacy"), None).await;
        assert_eq!(r.status, 401, "{}", t.label);
    }
}

async fn standard(t: &common::TestApi, token: &str, kind: &str) -> Value {
    let r = t.get(&format!("{BASE}/{kind}"), Some(token)).await;
    assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
    r.body["template"].clone()
}

#[tokio::test]
async fn rights_beat_validation_on_put() {
    for t in apis(Listen::Network).await {
        let admin = t.admin_token().await;
        let mut bad = standard(&t, &admin, "intake_act").await;
        bad["page"]["size"] = json!("a5");
        bad["layout"] = json!("two_per_a4");
        let plain = t.token_with("plain", &[Permission::OrdersCreate]).await;
        let r = t
            .send(
                Method::PUT,
                &format!("{BASE}/intake_act"),
                Some(&plain),
                Some(json!({"template": bad, "version": 0})),
            )
            .await;
        assert_eq!(r.status, 403, "{}: {}", t.label, r.body);
    }
}

#[tokio::test]
async fn put_and_file_upload_need_the_settings_right() {
    for t in apis(Listen::Network).await {
        let admin = t.admin_token().await;
        let template = standard(&t, &admin, "intake_act").await;
        let (_, _, file) = t.get_raw(&format!("{BASE}/intake_act/file"), &admin).await;
        let file: Value = serde_json::from_slice(&file).unwrap();
        let plain = t.token_with("plain", &[Permission::OrdersCreate]).await;
        let r = t
            .send(
                Method::PUT,
                &format!("{BASE}/intake_act"),
                Some(&plain),
                Some(json!({"template": template, "version": 0})),
            )
            .await;
        assert_eq!(r.status, 403, "{}: {}", t.label, r.body);
        let r = t
            .send(
                Method::POST,
                &format!("{BASE}/intake_act/file"),
                Some(&plain),
                Some(file),
            )
            .await;
        assert_eq!(r.status, 403, "{}: {}", t.label, r.body);
    }
}

#[tokio::test]
async fn unknown_kind_is_400() {
    for t in apis(Listen::Network).await {
        let admin = t.admin_token().await;
        let template = standard(&t, &admin, "intake_act").await;
        let r = t.get(&format!("{BASE}/no_such_kind"), Some(&admin)).await;
        assert_eq!(r.status, 400, "{}: {}", t.label, r.body);
        let r = t
            .send(
                Method::PUT,
                &format!("{BASE}/no_such_kind"),
                Some(&admin),
                Some(json!({"template": template, "version": 0})),
            )
            .await;
        assert_eq!(r.status, 400, "{}: {}", t.label, r.body);
        let r = t
            .send(
                Method::POST,
                &format!("{BASE}/no_such_kind/preview"),
                Some(&admin),
                Some(json!({"template": template})),
            )
            .await;
        assert_eq!(r.status, 400, "{}: {}", t.label, r.body);
    }
}

#[tokio::test]
async fn preview_of_another_kind_is_422() {
    for t in apis(Listen::Network).await {
        let admin = t.admin_token().await;
        let template = standard(&t, &admin, "intake_act").await;
        let r = t
            .send(
                Method::POST,
                &format!("{BASE}/issue_act/preview"),
                Some(&admin),
                Some(json!({"template": template})),
            )
            .await;
        assert_eq!(r.status, 422, "{}: {}", t.label, r.body);
        assert_eq!(r.body["fields"][0]["field"], "kind", "{}", t.label);
        assert_eq!(r.body["fields"][0]["code"], "mismatch", "{}", t.label);
    }
}

#[tokio::test]
async fn preview_with_order_id_uses_the_order() {
    for t in apis(Listen::Network).await {
        let admin = t.admin_token().await;
        let template = standard(&t, &admin, "intake_act").await;
        let order = common::create_order(&t, &admin, common::new_order("Иван", "", "A", "B")).await;
        let id = order["id"].as_i64().unwrap();
        let preview = |order_id: i64| {
            let (t, admin, template) = (&t, admin.clone(), template.clone());
            async move {
                t.send(
                    Method::POST,
                    &format!("{BASE}/intake_act/preview"),
                    Some(&admin),
                    Some(json!({"template": template, "order_id": order_id})),
                )
                .await
            }
        };
        let r = preview(id).await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        assert!(!r.body["pages"].as_array().unwrap().is_empty());
        // Заказ действительно берётся из базы: несуществующий даёт 404, а не образец.
        let r = preview(id + 1000).await;
        assert_eq!(r.status, 404, "{}: {}", t.label, r.body);
    }
}

#[tokio::test]
async fn preview_renders_the_draft_not_the_standard() {
    for t in apis(Listen::Network).await {
        let admin = t.admin_token().await;
        let standard = standard(&t, &admin, "intake_act").await;
        let mut draft = standard.clone();
        draft["page"]["size"] = json!("a5");
        draft["layout"] = json!("one_per_page");
        let preview = |template: Value| {
            let (t, admin) = (&t, admin.clone());
            async move {
                t.send(
                    Method::POST,
                    &format!("{BASE}/intake_act/preview"),
                    Some(&admin),
                    Some(json!({"template": template})),
                )
                .await
            }
        };
        let base = preview(standard).await;
        let changed = preview(draft).await;
        assert_eq!(base.status, 200, "{}: {}", t.label, base.body);
        assert_eq!(changed.status, 200, "{}: {}", t.label, changed.body);
        let width = |r: &common::TestResponse| r.body["width_mm"].as_f64().unwrap();
        assert!((width(&base) - 210.0).abs() < 0.01, "{}", t.label);
        assert!((width(&changed) - 148.0).abs() < 0.01, "{}", t.label);
    }
}

#[tokio::test]
async fn block_code_over_http() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        let r = t.get(&format!("{BASE}/intake_act"), Some(&token)).await;
        let template = r.body["template"].clone();
        let at = template["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .position(|b| {
                b["body"]["type"] == "text" && b["body"]["text"].as_str().unwrap().contains('{')
            })
            .unwrap();
        let url = format!("{BASE}/intake_act/block-code");
        let send = |token: String, body: Value| {
            let (t, url) = (&t, url.clone());
            async move { t.send(Method::POST, &url, Some(&token), Some(body)).await }
        };
        let r = send(token.clone(), json!({"template": template, "path": [at]})).await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        assert!(
            r.body["code"].as_str().unwrap().contains("v(\""),
            "{}",
            t.label
        );

        let r = send(
            token.clone(),
            json!({"template": template, "path": [at], "surprise": 1}),
        )
        .await;
        assert_eq!(r.status, 400, "{}: {}", t.label, r.body);

        let r = send(token.clone(), json!({"template": template, "path": [999]})).await;
        assert_eq!(r.status, 422, "{}: {}", t.label, r.body);
        assert_eq!(r.body["fields"][0]["field"], "path", "{}", t.label);
        assert_eq!(
            r.body["fields"][0]["code"], "code_unavailable",
            "{}",
            t.label
        );

        let plain = t.token_with("plain", &[Permission::OrdersCreate]).await;
        let r = send(plain, json!({"template": template, "path": [at]})).await;
        assert_eq!(r.status, 403, "{}: {}", t.label, r.body);
    }
}
