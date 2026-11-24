//! Настройки, статусы, справочники, точки обслуживания.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::Method;
use common::{Listen, apis, create_order, new_order, status_id_by_name};
use mw_domain::permission::Permission;
use serde_json::json;

#[tokio::test]
async fn settings_endpoints() {
    for t in apis(Listen::Network).await {
        let admin = t.admin_token().await;
        let clerk = t.token_with("kassa", &[Permission::OrdersCreate]).await;
        let r = t.get("/api/v1/settings", Some(&clerk)).await;
        assert_eq!(
            (
                r.status,
                &r.body["currency_symbol"],
                &r.body["default_warranty"]
            ),
            (200, &json!("₽"), &json!("30 дней")),
            "{}",
            t.label
        );
        let r = t
            .send(
                Method::PATCH,
                "/api/v1/settings",
                Some(&admin),
                Some(json!({ "default_warranty": "6 месяцев" })),
            )
            .await;
        assert_eq!(
            (r.status, &r.body["default_warranty"]),
            (200, &json!("6 месяцев"))
        );
        let r = t
            .send(
                Method::PATCH,
                "/api/v1/settings",
                Some(&admin),
                Some(json!({ "default_warranty_days": 30 })),
            )
            .await;
        assert_eq!(r.status, 400, "{}", t.label);
        let r = t
            .send(
                Method::PATCH,
                "/api/v1/settings",
                Some(&clerk),
                Some(json!({ "master_percent": 40 })),
            )
            .await;
        assert_eq!(
            (r.status, &r.body["code"], &r.body["permission"]),
            (403, &json!("forbidden"), &json!("settings_manage"))
        );
        let r = t
            .send(
                Method::PATCH,
                "/api/v1/settings",
                Some(&admin),
                Some(json!({ "master_percent": 101 })),
            )
            .await;
        assert_eq!(
            (r.status, &r.body["fields"]),
            (
                422,
                &json!([{ "field": "master_percent", "code": "out_of_range", "message": "Значение вне допустимого диапазона" }])
            )
        );
        assert_eq!(
            t.send(
                Method::PATCH,
                "/api/v1/settings",
                Some(&admin),
                Some(json!({ "master_percent": null }))
            )
            .await
            .status,
            400
        );
    }
}

#[tokio::test]
async fn status_endpoints() {
    for t in apis(Listen::Network).await {
        let admin = t.admin_token().await;
        assert_eq!(
            t.get("/api/v1/statuses", Some(&admin))
                .await
                .body
                .as_array()
                .unwrap()
                .len(),
            8
        );
        let r = t
            .send(
                Method::POST,
                "/api/v1/statuses",
                Some(&admin),
                Some(json!({ "name": "Выдан курьером", "kind": "issued", "color": "gray" })),
            )
            .await;
        assert_eq!((r.status, &r.body["kind"]), (201, &json!("issued")));
        let issued = status_id_by_name(&t.core, "Выдан").await;
        let r = t
            .send(
                Method::PATCH,
                &format!("/api/v1/statuses/{issued}"),
                Some(&admin),
                Some(json!({ "archived": true })),
            )
            .await;
        assert_eq!((r.status, &r.body["archived"]), (200, &json!(true)));
        assert_eq!(
            t.get("/api/v1/statuses", Some(&admin))
                .await
                .body
                .as_array()
                .unwrap()
                .len(),
            8
        );
        let all = t
            .get("/api/v1/statuses?include_archived=true", Some(&admin))
            .await
            .body;
        let ids: Vec<i64> = all
            .as_array()
            .unwrap()
            .iter()
            .rev()
            .map(|s| s["id"].as_i64().unwrap())
            .collect();
        assert_eq!(ids.len(), 9);
        assert_eq!(
            t.send(
                Method::PUT,
                "/api/v1/statuses/order",
                Some(&admin),
                Some(json!({ "ordered_ids": ids }))
            )
            .await
            .status,
            204
        );
        assert_eq!(
            t.get("/api/v1/statuses?include_archived=true", Some(&admin))
                .await
                .body[0]["id"],
            json!(ids[0])
        );
        let r = t
            .send(
                Method::PUT,
                "/api/v1/statuses/order",
                Some(&admin),
                Some(json!({ "ordered_ids": &ids[1..] })),
            )
            .await;
        assert_eq!(
            (r.status, &r.body["fields"][0]["field"]),
            (422, &json!("ordered_ids"))
        );
        let r = t
            .send(
                Method::PATCH,
                "/api/v1/statuses/999999",
                Some(&admin),
                Some(json!({ "color": "red" })),
            )
            .await;
        assert_eq!((r.status, &r.body["code"]), (404, &json!("not_found")));
        assert!(r.body["message"].as_str().unwrap().contains("статус"));
    }
}

#[tokio::test]
async fn dictionary_endpoints() {
    for t in apis(Listen::Network).await {
        let admin = t.admin_token().await;
        let clerk = t.token_with("kassa", &[Permission::OrdersCreate]).await;
        assert!(
            t.get("/api/v1/dictionaries/brand", Some(&clerk))
                .await
                .body
                .as_array()
                .unwrap()
                .iter()
                .any(|i| i["value"] == "Apple")
        );
        let r = t
            .send(
                Method::POST,
                "/api/v1/dictionaries/brand",
                Some(&clerk),
                Some(json!({ "value": "  Nothing " })),
            )
            .await;
        assert_eq!(
            (r.status, &r.body["value"], &r.body["kind"]),
            (201, &json!("Nothing"), &json!("brand"))
        );
        let id = r.body["id"].as_i64().unwrap();
        assert_eq!(
            t.send(
                Method::DELETE,
                &format!("/api/v1/dictionary-items/{id}"),
                Some(&clerk),
                None
            )
            .await
            .status,
            403
        );
        let r = t
            .send(
                Method::PATCH,
                &format!("/api/v1/dictionary-items/{id}"),
                Some(&admin),
                Some(json!({ "value": "Nothing Phone" })),
            )
            .await;
        assert_eq!(r.body["value"], json!("Nothing Phone"));
        assert_eq!(
            t.send(
                Method::DELETE,
                &format!("/api/v1/dictionary-items/{id}"),
                Some(&admin),
                None
            )
            .await
            .status,
            204
        );
        let colors: Vec<i64> = t
            .get("/api/v1/dictionaries/color", Some(&admin))
            .await
            .body
            .as_array()
            .unwrap()
            .iter()
            .rev()
            .map(|i| i["id"].as_i64().unwrap())
            .collect();
        assert_eq!(
            t.send(
                Method::PUT,
                "/api/v1/dictionaries/color/order",
                Some(&admin),
                Some(json!({ "ordered_ids": colors }))
            )
            .await
            .status,
            204
        );
        assert_eq!(
            t.get("/api/v1/dictionaries/color", Some(&admin)).await.body[0]["id"],
            json!(colors[0])
        );
        assert_eq!(
            t.get("/api/v1/dictionaries/planet", Some(&admin))
                .await
                .status,
            400
        );
    }
}

#[tokio::test]
async fn service_point_endpoints() {
    for t in apis(Listen::Network).await {
        let admin = t.admin_token().await;
        let r = t
            .send(
                Method::POST,
                "/api/v1/service-points",
                Some(&admin),
                Some(json!({ "name": "Древлянка" })),
            )
            .await;
        assert_eq!(r.status, 201);
        let id = r.body["id"].as_i64().unwrap();
        let r = t
            .send(
                Method::PATCH,
                &format!("/api/v1/service-points/{id}"),
                Some(&admin),
                Some(json!({ "archived": true })),
            )
            .await;
        assert_eq!(r.body["archived"], json!(true));
        assert_eq!(
            t.get("/api/v1/service-points", Some(&admin)).await.body,
            json!([])
        );
        assert_eq!(
            t.get("/api/v1/service-points?include_archived=true", Some(&admin))
                .await
                .body
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            t.send(
                Method::PUT,
                "/api/v1/service-points/order",
                Some(&admin),
                Some(json!({ "ordered_ids": [id] }))
            )
            .await
            .status,
            204
        );
    }
}

#[tokio::test]
async fn status_color_outside_the_palette_is_a_validation_error() {
    for t in apis(Listen::Network).await {
        let admin = t.admin_token().await;
        let r = t
            .send(
                Method::POST,
                "/api/v1/statuses",
                Some(&admin),
                Some(json!({ "name": "Цветной", "kind": "open", "color": "#ff0000" })),
            )
            .await;
        assert_eq!(
            (r.status, &r.body["fields"][0]),
            (
                422,
                &json!({ "field": "color", "code": "invalid",
                         "message": "Цвет статуса — одно из 12 имён палитры" })
            ),
            "{}",
            t.label
        );
    }
}

#[tokio::test]
async fn dictionary_archive_usage_next_number_and_about() {
    for t in apis(Listen::Network).await {
        let admin = t.admin_token().await;
        let clerk = t.token_with("kassa", &[Permission::OrdersCreate]).await;
        let items = t.get("/api/v1/dictionaries/brand", Some(&clerk)).await.body;
        let apple = items
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["value"] == "Apple")
            .unwrap()["id"]
            .as_i64()
            .unwrap();
        let path = format!("/api/v1/dictionary-items/{apple}");
        let r = t
            .send(
                Method::PATCH,
                &path,
                Some(&admin),
                Some(json!({ "archived": true })),
            )
            .await;
        assert_eq!((r.status, &r.body["archived"]), (200, &json!(true)));
        let r = t
            .send(
                Method::PATCH,
                &path,
                Some(&admin),
                Some(json!({ "archived": true, "extra": 1 })),
            )
            .await;
        assert_eq!(r.status, 400, "{}", t.label);
        let hidden = t.get("/api/v1/dictionaries/brand", Some(&clerk)).await.body;
        assert!(hidden.as_array().unwrap().iter().all(|i| i["id"] != apple));
        let shown = t
            .get("/api/v1/dictionaries/brand?archived=true", Some(&clerk))
            .await
            .body;
        assert!(shown.as_array().unwrap().iter().any(|i| i["id"] == apple));

        for path in [
            "/api/v1/dictionaries/brand/usage",
            "/api/v1/statuses/usage",
            "/api/v1/service-points/usage",
        ] {
            let r = t.get(path, Some(&admin)).await;
            assert_eq!((r.status, &r.body), (200, &json!([])), "{path}");
        }
        for path in ["/api/v1/statuses/usage", "/api/v1/service-points/usage"] {
            assert_eq!(t.get(path, Some(&clerk)).await.status, 403, "{path}");
        }
        // Счёт значений нужен приёму: «Комплектность» и «Состояние» — частые первыми.
        let r = t
            .get("/api/v1/dictionaries/brand/usage", Some(&clerk))
            .await;
        assert_eq!((r.status, &r.body), (200, &json!([])), "{}", t.label);
        let stock = t.token_with("sklad", &[Permission::StockView]).await;
        assert_eq!(
            t.get("/api/v1/dictionaries/brand/usage", Some(&stock))
                .await
                .status,
            403,
            "{}",
            t.label
        );
        assert_eq!(
            t.get("/api/v1/dictionaries/planet/usage", Some(&admin))
                .await
                .status,
            400
        );

        let r = t.get("/api/v1/orders/next-number", Some(&admin)).await;
        assert_eq!(
            (r.status, &r.body),
            (200, &json!({ "next": 1 })),
            "{}",
            t.label
        );
        assert_eq!(
            t.send(
                Method::PUT,
                "/api/v1/orders/next-number",
                Some(&admin),
                Some(json!({ "next": 500 }))
            )
            .await
            .status,
            204
        );
        assert_eq!(
            t.get("/api/v1/orders/next-number", Some(&admin)).await.body,
            json!({ "next": 500 })
        );
        assert_eq!(
            t.get("/api/v1/orders/next-number", Some(&clerk))
                .await
                .status,
            403
        );

        let r = t.get("/api/v1/about", Some(&admin)).await;
        assert_eq!(r.status, 200);
        assert_eq!(r.body["version"], json!(env!("CARGO_PKG_VERSION")));
        assert_eq!(
            (&r.body["orders"], &r.body["clients"]),
            (&json!(0), &json!(0))
        );
        assert!(r.body["database"] == "SQLite" || r.body["database"] == "PostgreSQL");
        assert!(r.body["data_dir"].is_string());
        assert_eq!(t.get("/api/v1/about", Some(&clerk)).await.status, 403);
        assert_eq!(t.get("/api/v1/about", None).await.status, 401);
    }
}

#[tokio::test]
async fn dictionary_rename_and_merge_endpoints() {
    for t in apis(Listen::Network).await {
        let admin = t.admin_token().await;
        let clerk = t
            .token_with("kassa", &[Permission::OrdersCreate, Permission::OrdersEdit])
            .await;
        let brand_id = |items: &serde_json::Value, value: &str| {
            items
                .as_array()
                .unwrap()
                .iter()
                .find(|i| i["value"] == value)
                .map(|i| i["id"].as_i64().unwrap())
        };
        let first = create_order(
            &t,
            &admin,
            new_order("Пётр", "+7 921 000-00-01", "samsung", "A52"),
        )
        .await;
        let items = t.get("/api/v1/dictionaries/brand", Some(&admin)).await.body;
        let samsung = brand_id(&items, "Samsung").unwrap();
        let rename = format!("/api/v1/dictionary-items/{samsung}/rename");

        let r = t
            .send(
                Method::POST,
                &rename,
                Some(&clerk),
                Some(json!({ "value": "Samsung Mobile", "in_orders": true })),
            )
            .await;
        assert_eq!(
            (r.status, &r.body["permission"]),
            (403, &json!("settings_manage")),
            "{}",
            t.label
        );
        for body in [
            json!({ "value": "Samsung Mobile", "in_orders": true, "extra": 1 }),
            json!({ "value": "Samsung Mobile" }),
        ] {
            let r = t
                .send(Method::POST, &rename, Some(&admin), Some(body))
                .await;
            assert_eq!(r.status, 400, "{}: {}", t.label, r.body);
        }
        let r = t
            .send(
                Method::POST,
                &rename,
                Some(&admin),
                Some(json!({ "value": "Samsung Mobile", "in_orders": true })),
            )
            .await;
        assert_eq!(
            (
                r.status,
                &r.body["item"]["id"],
                &r.body["item"]["value"],
                &r.body["records_changed"]
            ),
            (200, &json!(samsung), &json!("Samsung Mobile"), &json!(1)),
            "{}: {}",
            t.label,
            r.body
        );
        let order = t
            .get(&format!("/api/v1/orders/{}", first["id"]), Some(&admin))
            .await
            .body;
        assert_eq!(
            (&order["brand"], &order["version"]),
            (&json!("Samsung Mobile"), &json!(2))
        );

        let r = t
            .send(
                Method::POST,
                "/api/v1/dictionaries/brand",
                Some(&admin),
                Some(json!({ "value": "Самсунг" })),
            )
            .await;
        let cyrillic = r.body["id"].as_i64().unwrap();
        create_order(
            &t,
            &admin,
            new_order("Анна", "+7 921 000-00-02", "самсунг", "A53"),
        )
        .await;
        let merge = "/api/v1/dictionaries/brand/merge";
        let r = t
            .send(
                Method::POST,
                merge,
                Some(&admin),
                Some(json!({ "target_id": samsung, "source_ids": [], "in_orders": true })),
            )
            .await;
        assert_eq!(
            (r.status, &r.body["fields"]),
            (
                422,
                &json!([{ "field": "source_ids", "code": "required", "message": "Заполните поле" }])
            ),
            "{}",
            t.label
        );
        let body = json!({ "target_id": samsung, "source_ids": [cyrillic], "in_orders": true });
        assert_eq!(
            t.send(Method::POST, merge, Some(&clerk), Some(body.clone()))
                .await
                .status,
            403
        );
        assert_eq!(
            t.send(
                Method::POST,
                "/api/v1/dictionaries/planet/merge",
                Some(&admin),
                Some(body.clone())
            )
            .await
            .status,
            400
        );
        let r = t.send(Method::POST, merge, Some(&admin), Some(body)).await;
        assert_eq!(
            (
                r.status,
                &r.body["item"]["value"],
                &r.body["records_changed"]
            ),
            (200, &json!("Samsung Mobile"), &json!(1)),
            "{}: {}",
            t.label,
            r.body
        );
        let items = t.get("/api/v1/dictionaries/brand", Some(&admin)).await.body;
        assert_eq!(brand_id(&items, "Самсунг"), None);
        let page = t
            .get("/api/v1/orders?query=samsung%20mobile", Some(&admin))
            .await
            .body;
        assert_eq!(page["total"], json!(2), "{}: {page}", t.label);
    }
}

/// Отказы переименования называют причину: другой срок гарантии в заказах, запятая в списке.
/// Мастер и гарантия по умолчанию следуют за переименованием.
#[tokio::test]
async fn dictionary_rename_refusals_and_defaults() {
    for t in apis(Listen::Network).await {
        let admin = t.admin_token().await;
        // У заказа гарантия «30 дней» (как в new_order).
        create_order(
            &t,
            &admin,
            new_order("Пётр", "+7 921 000-00-11", "Samsung", "A52"),
        )
        .await;
        let id_of = |items: &serde_json::Value, value: &str| {
            items
                .as_array()
                .unwrap()
                .iter()
                .find(|i| i["value"] == value)
                .map(|i| i["id"].as_i64().unwrap())
                .unwrap()
        };
        let warranty = t
            .get("/api/v1/dictionaries/warranty", Some(&admin))
            .await
            .body;
        let thirty = id_of(&warranty, "30 дней");
        let rename = format!("/api/v1/dictionary-items/{thirty}/rename");
        let r = t
            .send(
                Method::POST,
                &rename,
                Some(&admin),
                Some(json!({ "value": "60 дней", "in_orders": true })),
            )
            .await;
        assert_eq!(
            (r.status, &r.body["fields"]),
            (
                422,
                &json!([{
                    "field": "value",
                    "code": "changes_term",
                    "message": "Срок гарантии в заказах изменился бы: замените значение только в справочнике"
                }])
            ),
            "{}: {}",
            t.label,
            r.body
        );
        let r = t
            .send(
                Method::POST,
                &rename,
                Some(&admin),
                Some(json!({ "value": "60 дней", "in_orders": false })),
            )
            .await;
        assert_eq!(
            (r.status, &r.body["records_changed"]),
            (200, &json!(0)),
            "{}",
            t.label
        );
        let settings = t.get("/api/v1/settings", Some(&admin)).await.body;
        assert_eq!(
            settings["default_warranty"],
            json!("60 дней"),
            "{}",
            t.label
        );

        let completeness = t
            .get("/api/v1/dictionaries/completeness", Some(&admin))
            .await
            .body;
        let cover = id_of(&completeness, "Чехол");
        let r = t
            .send(
                Method::PATCH,
                &format!("/api/v1/dictionary-items/{cover}"),
                Some(&admin),
                Some(json!({ "value": "Чехол, стекло" })),
            )
            .await;
        assert_eq!(
            (
                r.status,
                &r.body["fields"][0]["field"],
                &r.body["fields"][0]["code"]
            ),
            (422, &json!("value"), &json!("comma")),
            "{}",
            t.label
        );
    }
}

/// Перестановка со взглядом экрана (`seen_ids`): порядок изменили тем временем — 422 `stale`.
#[tokio::test]
async fn reorder_with_a_stale_view_is_refused() {
    for t in apis(Listen::Network).await {
        let admin = t.admin_token().await;
        let list = t
            .get("/api/v1/statuses?include_archived=true", Some(&admin))
            .await
            .body;
        let seen: Vec<i64> = list
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["id"].as_i64().unwrap())
            .collect();
        let mut first = seen.clone();
        first.rotate_right(1);
        let r = t
            .send(
                Method::PUT,
                "/api/v1/statuses/order",
                Some(&admin),
                Some(json!({ "ordered_ids": first, "seen_ids": seen })),
            )
            .await;
        assert_eq!(r.status, 204, "{}: {}", t.label, r.body);
        let mut second = seen.clone();
        second.swap(0, 1);
        let r = t
            .send(
                Method::PUT,
                "/api/v1/statuses/order",
                Some(&admin),
                Some(json!({ "ordered_ids": second, "seen_ids": seen })),
            )
            .await;
        assert_eq!(
            (r.status, &r.body["fields"]),
            (
                422,
                &json!([{
                    "field": "ordered_ids",
                    "code": "stale",
                    "message": "Порядок тем временем изменили: обновите список и повторите"
                }])
            ),
            "{}",
            t.label
        );
        // Без `seen_ids` — как раньше.
        let r = t
            .send(
                Method::PUT,
                "/api/v1/statuses/order",
                Some(&admin),
                Some(json!({ "ordered_ids": second })),
            )
            .await;
        assert_eq!(r.status, 204, "{}", t.label);
    }
}

/// Тот же `seen_ids` для точек и справочников: свежий взгляд проходит, устаревший — 422 `stale`,
/// без него — как раньше.
#[tokio::test]
async fn reorder_with_a_stale_view_is_refused_for_points_and_dictionaries() {
    for t in apis(Listen::Network).await {
        let admin = t.admin_token().await;
        for name in ["Древлянка", "Центр", "Кукковка"] {
            let r = t
                .send(
                    Method::POST,
                    "/api/v1/service-points",
                    Some(&admin),
                    Some(json!({ "name": name })),
                )
                .await;
            assert_eq!(r.status, 201, "{}", t.label);
        }
        for (list, order) in [
            (
                "/api/v1/service-points?include_archived=true",
                "/api/v1/service-points/order",
            ),
            (
                "/api/v1/dictionaries/brand?archived=true",
                "/api/v1/dictionaries/brand/order",
            ),
        ] {
            let seen: Vec<i64> = t
                .get(list, Some(&admin))
                .await
                .body
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s["id"].as_i64().unwrap())
                .collect();
            assert!(seen.len() >= 3, "{}: {order}", t.label);
            let mut first = seen.clone();
            first.rotate_right(1);
            let put = |ordered: &[i64], seen: Option<&[i64]>| {
                let mut body = json!({ "ordered_ids": ordered });
                if let Some(seen) = seen {
                    body["seen_ids"] = json!(seen);
                }
                t.send(Method::PUT, order, Some(&admin), Some(body))
            };
            let r = put(&first, Some(&seen)).await;
            assert_eq!(r.status, 204, "{}: {order}: {}", t.label, r.body);
            let mut second = seen.clone();
            second.swap(0, 1);
            let r = put(&second, Some(&seen)).await;
            assert_eq!(
                (r.status, &r.body["fields"][0]["code"]),
                (422, &json!("stale")),
                "{}: {order}",
                t.label
            );
            // Взгляд свежий (после первой перестановки) — проходит.
            let r = put(&second, Some(&first)).await;
            assert_eq!(r.status, 204, "{}: {order}: {}", t.label, r.body);
            // Без `seen_ids` — как раньше.
            let r = put(&first, None).await;
            assert_eq!(r.status, 204, "{}: {order}", t.label);
        }
    }
}
