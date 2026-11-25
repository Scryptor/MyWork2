//! Резервные копии по HTTP: только сервер на этом компьютере, настройки, ручная копия,
//! восстановление при запуске, права.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use std::time::{Duration, Instant};

use axum::http::Method;
use common::*;
use mw_domain::permission::Permission;
use serde_json::{Value, json};

const BACKUPS: &str = "/api/v1/backups";
const RESTORE: &str = "/api/v1/backups/restore";
const UNAVAILABLE_ON_PG: &str =
    "Для PostgreSQL резервные копии делает pg_dump по расписанию на сервере";

#[tokio::test]
async fn backups_are_only_for_this_computer() {
    for t in backup_apis(Listen::Network).await {
        let token = t.admin_token().await;
        let calls = [
            (Method::GET, BACKUPS.to_owned(), None),
            (
                Method::PATCH,
                format!("{BACKUPS}/settings"),
                Some(json!({ "keep": 5 })),
            ),
            (Method::POST, BACKUPS.to_owned(), None),
            (Method::DELETE, format!("{BACKUPS}/x"), None),
            (
                Method::POST,
                RESTORE.to_owned(),
                Some(json!({ "name": "x" })),
            ),
            (Method::DELETE, RESTORE.to_owned(), None),
            (
                Method::POST,
                format!("{BACKUPS}/legacy"),
                Some(json!({ "apply": true })),
            ),
        ];
        for (method, path, body) in calls {
            let r = t.send(method.clone(), &path, Some(&token), body).await;
            assert_eq!(r.status, 403, "{} {method} {path}: {}", t.label, r.body);
            assert_eq!(
                r.body["message"],
                "Резервные копии настраиваются на компьютере, где работает сервер"
            );
        }
    }
}

#[tokio::test]
async fn overview_settings_and_errors() {
    for t in backup_apis(Listen::Loopback).await {
        let token = t.admin_token().await;
        let r = t.get(BACKUPS, Some(&token)).await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        if t.label != "sqlite" {
            assert_eq!(r.body["supported"], false, "{}", t.label);
            assert_eq!(r.body["reason"], UNAVAILABLE_ON_PG);
            let r = t.send(Method::POST, BACKUPS, Some(&token), None).await;
            assert_eq!(r.status, 503, "{}: {}", t.label, r.body);
            continue;
        }
        assert_eq!(
            (&r.body["supported"], &r.body["settings"]["keep"]),
            (&json!(true), &json!(10))
        );
        let settings = format!("{BACKUPS}/settings");
        let r = t
            .send(
                Method::PATCH,
                &settings,
                Some(&token),
                Some(json!({ "keep": 0 })),
            )
            .await;
        assert_eq!(r.status, 422, "{}", r.body);
        assert_eq!(
            (&r.body["fields"][0]["field"], &r.body["fields"][0]["code"]),
            (&json!("keep"), &json!("out_of_range"))
        );
        let r = t
            .send(
                Method::PATCH,
                &settings,
                Some(&token),
                Some(json!({ "folder": "relative" })),
            )
            .await;
        assert_eq!(r.status, 422, "{}", r.body);
        assert_eq!(r.body["fields"][0]["code"], "folder_relative");
        assert_eq!(
            r.body["fields"][0]["message"],
            "Укажите полный путь к папке"
        );
        let r = t
            .send(
                Method::PATCH,
                &settings,
                Some(&token),
                Some(json!({ "kep": 1 })),
            )
            .await;
        assert_eq!(r.status, 400, "{}", r.body);
        let r = t
            .send(
                Method::PATCH,
                &settings,
                Some(&token),
                Some(json!({ "keep": 3 })),
            )
            .await;
        assert_eq!((r.status, &r.body["keep"]), (200, &json!(3)), "{}", r.body);
    }
}

#[tokio::test]
async fn manual_copy_restore_and_cancel() {
    for t in backup_apis(Listen::Loopback).await {
        if t.label != "sqlite" {
            continue;
        }
        let token = t.admin_token().await;
        let r = t.send(Method::POST, BACKUPS, Some(&token), None).await;
        assert_eq!(r.status, 202, "{}", r.body);
        assert_eq!(r.body["finished_at"], Value::Null);
        let started = Instant::now();
        let overview = loop {
            let r = t.get(BACKUPS, Some(&token)).await;
            if r.body["job"]["finished_at"] != Value::Null {
                break r.body;
            }
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "копия не закончилась"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        };
        assert_eq!(overview["job"]["error"], Value::Null, "{overview}");
        let name = overview["copies"][0]["name"].as_str().unwrap().to_owned();
        assert_eq!(overview["copies"].as_array().unwrap().len(), 1);

        let r = t
            .send(
                Method::POST,
                RESTORE,
                Some(&token),
                Some(json!({ "name": name })),
            )
            .await;
        assert_eq!(r.status, 202, "{}", r.body);
        let r = t.get(BACKUPS, Some(&token)).await;
        assert_eq!(
            r.body["pending"]["copy"]["name"],
            name.as_str(),
            "{}",
            r.body
        );

        let r = t.send(Method::DELETE, RESTORE, Some(&token), None).await;
        assert_eq!(r.status, 204, "{}", r.body);
        let r = t.get(BACKUPS, Some(&token)).await;
        assert_eq!(r.body["pending"], Value::Null);

        let r = t
            .send(
                Method::DELETE,
                &format!("{BACKUPS}/{name}"),
                Some(&token),
                None,
            )
            .await;
        assert_eq!(r.status, 204, "{}", r.body);
        let r = t
            .send(
                Method::DELETE,
                &format!("{BACKUPS}/nope"),
                Some(&token),
                None,
            )
            .await;
        assert_eq!(r.status, 422, "{}", r.body);
        assert_eq!(r.body["fields"][0]["code"], "backup_missing");
    }
}

#[tokio::test]
async fn restore_and_delete_need_users_manage() {
    for t in backup_apis(Listen::Loopback).await {
        let token = t.token_with("weak", &[Permission::SettingsManage]).await;
        let r = t
            .send(
                Method::POST,
                RESTORE,
                Some(&token),
                Some(json!({ "name": "x" })),
            )
            .await;
        assert_eq!(r.status, 403, "{}: {}", t.label, r.body);
        let r = t
            .send(Method::DELETE, &format!("{BACKUPS}/x"), Some(&token), None)
            .await;
        assert_eq!(r.status, 403, "{}: {}", t.label, r.body);
    }
}
