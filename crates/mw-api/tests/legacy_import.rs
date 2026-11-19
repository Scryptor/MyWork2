//! Импорт из MyWork 2 через API: проверка папки, перенос фоновой задачей с ходом по этапам, отчёты.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use std::time::Duration;

use axum::http::Method;
use common::*;
use mw_core::{Actor, NewClient};
use mw_domain::permission::Permission;
use mw_legacy::ImportStage;
use mw_legacy::fixture::{FixtureOptions, FixtureVersion, write_fixture};
use mw_legacy::testing::hold;
use serde_json::{Value, json};
use tempfile::TempDir;

const CHECK: &str = "/api/v1/legacy-import/check";
const START: &str = "/api/v1/legacy-import";
const CURRENT: &str = "/api/v1/legacy-import/current";
const LIST: &str = "/api/v1/legacy-imports";

async fn fixture(extra_orders: usize) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    write_fixture(
        dir.path(),
        &FixtureOptions {
            version: FixtureVersion::V32,
            extra_orders,
        },
    )
    .await
    .unwrap();
    dir
}

fn path_of(dir: &TempDir) -> String {
    dir.path().to_string_lossy().into_owned()
}

/// Вход пользователем из фикстуры MyWork 2 (группа «Админы»).
async fn login_imported_admin(t: &TestApi) -> String {
    let r = t
        .send(
            Method::POST,
            "/api/v1/auth/login",
            None,
            Some(json!({ "login": "Ирина", "password": "1234", "device_label": "тест" })),
        )
        .await;
    assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
    r.body["token"].as_str().unwrap().to_owned()
}

/// Опрашивает ход до конца. После записи базы с пользователями MyWork 2 режим без входа кончается:
/// тогда опрос получает 401, входит пользователем из фикстуры и продолжает.
async fn wait_finished(t: &TestApi, token: &mut Option<String>) -> Value {
    for _ in 0..1000 {
        let r = t.get(CURRENT, token.as_deref()).await;
        if r.status == 401 && token.is_none() {
            *token = Some(login_imported_admin(t).await);
            continue;
        }
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        assert!(r.body.is_object(), "{}: {}", t.label, r.body);
        if r.body["stage"] == "done" || !r.body["error"].is_null() {
            return r.body;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("{}: импорт не закончился", t.label);
}

#[tokio::test]
async fn import_runs_in_the_background_and_its_report_is_listed() {
    for t in apis(Listen::Loopback).await {
        let src = fixture(0).await;
        assert_eq!(t.get(CURRENT, None).await.body, Value::Null, "{}", t.label);
        let check = t
            .send(
                Method::POST,
                CHECK,
                None,
                Some(json!({ "path": path_of(&src) })),
            )
            .await;
        assert_eq!(check.status, 200, "{}: {}", t.label, check.body);
        assert_eq!(
            (
                &check.body["ok"],
                &check.body["orders"],
                &check.body["files"]
            ),
            (&json!(true), &json!(11), &json!(6)),
            "{}",
            check.body
        );

        let started = t
            .send(
                Method::POST,
                START,
                None,
                Some(json!({ "path": path_of(&src), "replace": false })),
            )
            .await;
        assert_eq!(started.status, 202, "{}: {}", t.label, started.body);
        assert_eq!(started.body["stage"], "reading");
        assert!(started.body["error"].is_null() && started.body["import_id"].is_null());

        let mut token = None;
        let job = wait_finished(&t, &mut token).await;
        assert!(job["error"].is_null(), "{}: {job}", t.label);
        assert_eq!(job["percent"], 100);
        let id = job["import_id"].as_i64().expect("номер импорта");
        let token = match token {
            Some(token) => token,
            None => login_imported_admin(&t).await,
        };

        let list = t.get(LIST, Some(&token)).await;
        assert_eq!(list.status, 200, "{}: {}", t.label, list.body);
        assert_eq!(list.body.as_array().unwrap().len(), 1);
        let item = &list.body[0];
        assert_eq!(
            (
                &item["id"],
                &item["mode"],
                &item["orders"],
                &item["warnings"]
            ),
            (&json!(id), &json!("into_empty"), &json!(11), &json!(27)),
            "{item}"
        );
        assert_eq!(item["source_path"], json!(path_of(&src)));

        let report = t.get(&format!("{LIST}/{id}"), Some(&token)).await;
        assert_eq!(report.status, 200, "{}: {}", t.label, report.body);
        let r = &report.body;
        assert_eq!(r["orders"], 11);
        assert_eq!(r["files_pending"], json!(false), "{}", t.label);
        assert_eq!(
            (&r["files"]["copied"], &r["files"]["missing"]),
            (&json!(6), &json!(1))
        );
        let groups = r["groups"].as_array().unwrap();
        let dates = groups
            .iter()
            .find(|g| g["code"] == "date_unparsed")
            .expect("группа «Не разобрана дата»");
        assert_eq!(
            (&dates["title"], &dates["severity"]),
            (&json!("Не разобрана дата"), &json!("warning"))
        );
        assert!(
            !dates["order_ids"].as_array().unwrap().is_empty(),
            "{dates}"
        );

        // Тот же текст, что лежит в файле отчёта в папке данных.
        let reports = t.core.data_dir_path().unwrap().join("import-reports");
        let text_file = std::fs::read_dir(&reports)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| p.extension().is_some_and(|e| e == "txt"))
            .expect("файл отчёта");
        assert_eq!(
            r["warnings_text"],
            json!(std::fs::read_to_string(text_file).unwrap())
        );
        assert_eq!(
            t.get(&format!("{LIST}/{}", id + 100), Some(&token))
                .await
                .status,
            404
        );
    }
}

#[tokio::test]
async fn second_start_while_running_is_refused() {
    for t in apis(Listen::Loopback).await {
        let src = fixture(0).await;
        let held = hold(src.path(), ImportStage::Copying);
        let body = json!({ "path": path_of(&src), "replace": false });
        let first = t.send(Method::POST, START, None, Some(body.clone())).await;
        assert_eq!(first.status, 202, "{}: {}", t.label, first.body);
        held.reached.await.unwrap();
        let second = t.send(Method::POST, START, None, Some(body)).await;
        assert_eq!(second.status, 409, "{}: {}", t.label, second.body);
        assert_eq!(
            (&second.body["code"], &second.body["message"]),
            (&json!("conflict"), &json!("Импорт уже идёт"))
        );
        // Второй видит тот же ход.
        assert_eq!(t.get(CURRENT, None).await.body["stage"], "copying");
        held.release.send(()).unwrap();
        let job = wait_finished(&t, &mut None).await;
        assert!(job["error"].is_null(), "{}: {job}", t.label);
    }
}

/// Промежуточные папки `.import-*` в папке вложений.
fn staging_dirs(t: &TestApi) -> Vec<String> {
    let att = t.core.data_dir_path().unwrap().join("attachments");
    let Ok(entries) = std::fs::read_dir(att) else {
        return Vec::new();
    };
    entries
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(".import"))
        .collect()
}

/// Остановка сервера (`Server::serve` после последнего запроса) — `Core::wait_background`.
fn begin_shutdown(t: &TestApi) -> tokio::task::JoinHandle<bool> {
    let core = t.core.clone();
    tokio::spawn(async move { core.wait_background(Duration::from_secs(30)).await })
}

async fn wait_cancelled(t: &TestApi) {
    for _ in 0..500 {
        if t.core.shutdown_token().is_cancelled() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("{}: остановка не началась", t.label);
}

#[tokio::test]
async fn shutdown_while_copying_cancels_the_import_and_leaves_nothing() {
    for t in apis(Listen::Loopback).await {
        let src = fixture(0).await;
        let held = hold(src.path(), ImportStage::Copying);
        let body = json!({ "path": path_of(&src), "replace": false });
        let started = t.send(Method::POST, START, None, Some(body)).await;
        assert_eq!(started.status, 202, "{}: {}", t.label, started.body);
        held.reached.await.unwrap();
        let shutdown = begin_shutdown(&t);
        wait_cancelled(&t).await;
        let released = std::time::Instant::now();
        held.release.send(()).unwrap();
        let waited = tokio::time::timeout(Duration::from_secs(5), shutdown)
            .await
            .expect("остановка не дождалась отмены")
            .unwrap();
        assert!(waited, "{}", t.label);
        assert!(released.elapsed() < Duration::from_secs(5));
        let job = t.get(CURRENT, None).await.body;
        assert_eq!(
            (&job["stage"], &job["error"], &job["import_id"]),
            (
                &json!("copying"),
                &json!("Импорт прерван: программа закрывается"),
                &Value::Null
            ),
            "{}",
            t.label
        );
        assert!(t.core.import_target_state().await.unwrap().is_empty());
        assert!(
            t.core
                .list_legacy_imports(&Actor::unrestricted("Тест"))
                .await
                .unwrap()
                .is_empty()
        );
        assert!(staging_dirs(&t).is_empty(), "{}", t.label);
        let att = t.core.data_dir_path().unwrap().join("attachments");
        assert!(!att.join("orders").exists());
    }
}

#[tokio::test]
async fn shutdown_while_writing_waits_for_the_commit() {
    for t in apis(Listen::Loopback).await {
        let src = fixture(0).await;
        let held = hold(src.path(), ImportStage::Writing);
        let body = json!({ "path": path_of(&src), "replace": false });
        let started = t.send(Method::POST, START, None, Some(body)).await;
        assert_eq!(started.status, 202, "{}: {}", t.label, started.body);
        held.reached.await.unwrap();
        let shutdown = begin_shutdown(&t);
        wait_cancelled(&t).await;
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(
            !shutdown.is_finished(),
            "{}: остановка не ждёт записи",
            t.label
        );
        held.release.send(()).unwrap();
        let waited = tokio::time::timeout(Duration::from_secs(30), shutdown)
            .await
            .expect("остановка не закончилась")
            .unwrap();
        assert!(waited, "{}", t.label);
        let job = wait_finished(&t, &mut None).await;
        assert!(job["error"].is_null(), "{}: {job}", t.label);
        assert!(job["import_id"].is_i64());
        assert_eq!(t.core.import_target_state().await.unwrap().orders, 11);
        let att = t.core.data_dir_path().unwrap().join("attachments");
        assert!(att.join("orders/1/photo.jpg").exists() && staging_dirs(&t).is_empty());
    }
}

#[cfg(unix)]
#[tokio::test]
async fn files_not_placed_gives_the_error_and_the_import_number() {
    for t in apis(Listen::Loopback).await {
        let att = t.core.data_dir_path().unwrap().join("attachments");
        std::fs::create_dir_all(&att).unwrap();
        // Висячая ссылка: базу записать можно, а поставить файлы на место — нет.
        std::os::unix::fs::symlink(att.join("nowhere"), att.join("orders")).unwrap();
        let src = fixture(0).await;
        let body = json!({ "path": path_of(&src), "replace": false });
        let started = t.send(Method::POST, START, None, Some(body)).await;
        assert_eq!(started.status, 202, "{}: {}", t.label, started.body);
        let mut token = None;
        let job = wait_finished(&t, &mut token).await;
        let error = job["error"].as_str().expect("ошибка установки файлов");
        assert_eq!(
            error,
            "Файлы не удалось поставить на место — перезапустите программу, она закончит перенос сама",
            "{}",
            t.label
        );
        let id = job["import_id"].as_i64().expect("номер импорта");
        let list = t.get(LIST, token.as_deref()).await;
        assert_eq!(list.body[0]["id"], id, "{}: {}", t.label, list.body);
        assert_eq!(staging_dirs(&t).len(), 1, "{}", t.label);
        let report = t.get(&format!("{LIST}/{id}"), token.as_deref()).await;
        assert_eq!(report.status, 200, "{}: {}", t.label, report.body);
        assert_eq!(report.body["files_pending"], json!(true), "{}", t.label);
    }
}

#[tokio::test]
async fn check_names_an_occupied_attachments_folder_unless_replacing() {
    for t in apis(Listen::Loopback).await {
        let occupied = t.core.data_dir_path().unwrap().join("attachments").join("stock");
        std::fs::create_dir_all(occupied.join("1")).unwrap();
        std::fs::write(occupied.join("1/user.jpg"), b"user").unwrap();
        let src = fixture(0).await;
        let r = t
            .send(
                Method::POST,
                CHECK,
                None,
                Some(json!({ "path": path_of(&src) })),
            )
            .await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        assert_eq!(r.body["ok"], json!(false), "{}", t.label);
        let problem = r.body["problem"].as_str().unwrap();
        assert!(
            problem.contains(&occupied.display().to_string()),
            "{problem}"
        );
        let r = t
            .send(
                Method::POST,
                CHECK,
                None,
                Some(json!({ "path": path_of(&src), "replace": true })),
            )
            .await;
        assert_eq!(r.body["ok"], json!(true), "{}: {}", t.label, r.body);
    }
}

#[tokio::test]
async fn not_empty_database_without_replace_ends_with_a_hint() {
    for t in apis(Listen::Loopback).await {
        t.core
            .create_client(
                &Actor::unrestricted("Тест"),
                NewClient {
                    full_name: "Уже есть".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let src = fixture(0).await;
        let body = json!({ "path": path_of(&src), "replace": false });
        let started = t.send(Method::POST, START, None, Some(body.clone())).await;
        assert_eq!(started.status, 202, "{}: {}", t.label, started.body);
        let job = wait_finished(&t, &mut None).await;
        let error = job["error"].as_str().expect("ошибка импорта");
        assert!(
            error.contains("Импорт делается только в пустую базу") && error.contains("с заменой"),
            "{}: {error}",
            t.label
        );
        assert!(job["import_id"].is_null());
        // После ошибки можно начать заново.
        let again = t.send(Method::POST, START, None, Some(body)).await;
        assert_eq!(again.status, 202, "{}: {}", t.label, again.body);
        wait_finished(&t, &mut None).await;
    }
}

#[tokio::test]
async fn wrong_folder_is_explained_and_bad_bodies_are_refused() {
    for t in apis(Listen::Loopback).await {
        let empty = tempfile::tempdir().unwrap();
        let r = t
            .send(
                Method::POST,
                CHECK,
                None,
                Some(json!({ "path": path_of(&empty) })),
            )
            .await;
        assert_eq!(r.status, 200, "{}: {}", t.label, r.body);
        assert_eq!(r.body["ok"], false);
        assert!(
            r.body["problem"]
                .as_str()
                .unwrap()
                .contains("В папке нет baza.sqlite"),
            "{}",
            r.body
        );
        let r = t
            .send(
                Method::POST,
                CHECK,
                None,
                Some(json!({ "path": path_of(&empty), "extra": 1 })),
            )
            .await;
        assert_eq!(r.status, 400, "{}: {}", t.label, r.body);
        let r = t
            .send(
                Method::POST,
                START,
                None,
                Some(json!({ "path": "MyWork2", "replace": false })),
            )
            .await;
        assert_eq!(r.status, 400, "{}: {}", t.label, r.body);
        assert!(
            r.body["message"]
                .as_str()
                .unwrap()
                .contains("Укажите полный путь"),
            "{}",
            r.body
        );
        assert_eq!(t.get(CURRENT, None).await.body, Value::Null);
    }
}

#[tokio::test]
async fn path_import_is_only_for_a_server_on_this_computer() {
    for t in apis(Listen::Network).await {
        let token = t.admin_token().await;
        let calls = [
            (Method::POST, CHECK, Some(json!({ "path": "/old" }))),
            (
                Method::POST,
                START,
                Some(json!({ "path": "/old", "replace": false })),
            ),
            (Method::GET, CURRENT, None),
        ];
        for (method, path, body) in calls {
            let r = t.send(method, path, Some(&token), body).await;
            assert_eq!(r.status, 403, "{} {path}: {}", t.label, r.body);
            assert_eq!(
                r.body["message"],
                "Импорт по пути доступен только на этом компьютере"
            );
        }
        // Прошлые отчёты читаются и с сетевого сервера.
        let r = t.get(LIST, Some(&token)).await;
        assert_eq!((r.status, &r.body), (200, &json!([])), "{}", t.label);
    }
}

#[tokio::test]
async fn import_needs_settings_and_users_rights() {
    for t in apis(Listen::Loopback).await {
        let token = t.token_with("weak", &[Permission::SettingsManage]).await;
        let calls = [
            (Method::POST, CHECK, Some(json!({ "path": "/old" }))),
            (
                Method::POST,
                START,
                Some(json!({ "path": "/old", "replace": false })),
            ),
            (Method::GET, CURRENT, None),
            (Method::GET, LIST, None),
            (Method::GET, "/api/v1/legacy-imports/1", None),
        ];
        for (method, path, body) in calls {
            let r = t.send(method, path, Some(&token), body).await;
            assert_eq!(r.status, 403, "{} {path}: {}", t.label, r.body);
            assert_eq!(r.body["permission"], "users_manage", "{path}");
        }
    }
}
