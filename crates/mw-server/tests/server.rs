#![allow(clippy::unwrap_used, clippy::expect_used)]

use mw_server::{Config, Server, create_admin};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

async fn http(
    addr: std::net::SocketAddr,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> (u16, String) {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let body = body.unwrap_or("");
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await.unwrap();
    let text = String::from_utf8_lossy(&raw).to_string();
    let status = text.split(' ').nth(1).unwrap().parse().unwrap();
    let body = text
        .split_once("\r\n\r\n")
        .map(|x| x.1)
        .unwrap_or("")
        .to_string();
    (status, body)
}

fn temp_dir(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "mw-server-{tag}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn config(dir: PathBuf) -> Config {
    Config {
        listen: "127.0.0.1:0".parse().unwrap(),
        data_dir: dir,
        database_url: None,
        timezone: Some("UTC".into()),
        ui_dir: None,
    }
}

async fn stop(
    stop: tokio::sync::oneshot::Sender<()>,
    running: tokio::task::JoinHandle<Result<(), mw_server::ServerError>>,
    addr: std::net::SocketAddr,
    label: &str,
) {
    stop.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), running)
        .await
        .expect("сервер не остановился")
        .unwrap()
        .unwrap();
    assert!(TcpStream::connect(addr).await.is_err(), "{label}");
}

#[tokio::test]
async fn serves_api_and_stops_gracefully_on_every_backend() {
    for (label, db) in mw_db::testing::all_backends().await {
        let server = Server::start_with_db(&config(temp_dir(label)), db)
            .await
            .unwrap();
        let addr = server.local_addr();
        let (tx, stopped) = tokio::sync::oneshot::channel::<()>();
        let running = tokio::spawn(server.serve(async {
            stopped.await.ok();
        }));
        let (status, body) = http(addr, "GET", "/api/v1/server-info", None).await;
        assert_eq!(status, 200, "{label}: {body}");
        assert!(body.contains("\"auth_required\":false"), "{label}: {body}");
        stop(tx, running, addr, label).await;
    }
}

// Файловая SQLite — только SQLite (исключение плана для проверки папки данных).
#[tokio::test]
async fn creates_sqlite_database_in_data_dir_and_reopens_it() {
    let dir = temp_dir("file").join("данные с пробелом");
    let server = Server::start(&config(dir.clone())).await.unwrap();
    assert!(dir.join("mywork.db").exists());
    let addr = server.local_addr();
    let (tx, stopped) = tokio::sync::oneshot::channel::<()>();
    let running = tokio::spawn(server.serve(async {
        stopped.await.ok();
    }));
    let (status, body) = http(addr, "GET", "/api/v1/server-info", None).await;
    assert_eq!(status, 200, "{body}");
    stop(tx, running, addr, "file").await;
    Server::start(&config(dir.clone())).await.unwrap();
}

#[tokio::test]
async fn refuses_database_from_newer_version() {
    let dir = temp_dir("newer");
    drop(Server::start(&config(dir.clone())).await.unwrap());
    let db = mw_db::connect(&mw_db::sqlite_url(&dir.join("mywork.db")))
        .await
        .unwrap();
    db.execute_unprepared(
        "INSERT INTO seaql_migrations (version, applied_at) VALUES ('m29991231_000001_future', 0)",
    )
    .await
    .unwrap();
    db.close().await.unwrap();
    let err = Server::start(&config(dir.clone()))
        .await
        .err()
        .expect("старая программа не должна открыть новую базу");
    assert!(err.to_string().contains("миграц"), "{err}");
    let db = mw_db::connect(&mw_db::sqlite_url(&dir.join("mywork.db")))
        .await
        .unwrap();
    let row = db
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT COUNT(*) FROM seaql_migrations WHERE version = 'm29991231_000001_future'",
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.try_get_by_index::<i64>(0).unwrap(), 1);
}

#[tokio::test]
async fn create_admin_then_login_over_http() {
    let server = Server::start(&config(temp_dir("admin"))).await.unwrap();
    let user = create_admin(server.core(), "boss", "Шеф", "пароль-1234")
        .await
        .unwrap();
    assert_eq!(user.display_name, "Шеф");
    assert!(
        create_admin(server.core(), "BOSS", "", "пароль-1234")
            .await
            .is_err()
    );
    let addr = server.local_addr();
    let (tx, stopped) = tokio::sync::oneshot::channel::<()>();
    let running = tokio::spawn(server.serve(async {
        stopped.await.ok();
    }));
    let (status, body) = http(
        addr,
        "POST",
        "/api/v1/auth/login",
        Some(r#"{"login":"boss","password":"пароль-1234","device_label":"тест"}"#),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("\"token\""));
    stop(tx, running, addr, "admin").await;
}

#[tokio::test]
async fn connection_error_does_not_leak_the_password() {
    let mut cfg = config(temp_dir("leak"));
    cfg.database_url = Some("postgres://mw:Pa55#word@db.invalid:5432/mw".into());
    let err = Server::start(&cfg)
        .await
        .err()
        .expect("подключение должно не удаться");
    assert!(!err.to_string().contains("Pa55#word"), "{err}");
    assert!(!format!("{err:?}").contains("Pa55#word"), "{err:?}");
}

#[tokio::test]
async fn explicit_timezone_is_stored_on_first_start() {
    for (label, db) in mw_db::testing::all_backends().await {
        let mut cfg = config(temp_dir(label));
        cfg.timezone = Some("Europe/Moscow".into());
        let server = Server::start_with_db(&cfg, db).await.unwrap();
        assert_eq!(
            server.core().settings().timezone,
            "Europe/Moscow",
            "{label}"
        );
    }
}

#[tokio::test]
async fn schemeless_database_url_does_not_leak_the_password() {
    let mut cfg = config(temp_dir("schemeless"));
    cfg.database_url = Some("mw:secret@localhost:5432/mw".into());
    let err = Server::start(&cfg)
        .await
        .err()
        .expect("подключение должно не удаться");
    assert!(!err.to_string().contains("secret"), "{err}");
}

#[tokio::test]
async fn unknown_timezone_is_rejected_by_the_library() {
    for (label, db) in mw_db::testing::all_backends().await {
        let mut cfg = config(temp_dir(label));
        cfg.timezone = Some("Moscow".into());
        let err = Server::start_with_db(&cfg, db)
            .await
            .err()
            .expect("неизвестный пояс должен быть отвергнут");
        assert!(
            err.to_string()
                .contains("неизвестный часовой пояс «Moscow»"),
            "{label}: {err}"
        );
    }
}

#[tokio::test]
async fn missing_ui_dir_stops_the_start_with_its_path() {
    let missing = temp_dir("ui-missing").join("нет такой");
    let mut cfg = config(temp_dir("ui-missing-data"));
    cfg.ui_dir = Some(missing.clone());
    let err = Server::start(&cfg).await.err().expect("папки нет");
    assert_eq!(
        err.to_string(),
        format!("папка интерфейса не найдена: {}", missing.display())
    );
    let empty = temp_dir("ui-empty");
    cfg.ui_dir = Some(empty.clone());
    let err = Server::start(&cfg).await.err().expect("нет index.html");
    assert!(
        err.to_string().contains(&empty.display().to_string()),
        "{err}"
    );
}

#[tokio::test]
async fn serves_the_ui_from_the_same_origin() {
    let ui = temp_dir("ui");
    std::fs::write(ui.join("index.html"), "<title>mw</title>").unwrap();
    let mut cfg = config(temp_dir("ui-data"));
    cfg.ui_dir = Some(ui);
    let server = Server::start(&cfg).await.unwrap();
    let addr = server.local_addr();
    let (tx, stopped) = tokio::sync::oneshot::channel::<()>();
    let running = tokio::spawn(server.serve(async {
        stopped.await.ok();
    }));
    let (status, body) = http(addr, "GET", "/orders/1", None).await;
    assert_eq!((status, body.as_str()), (200, "<title>mw</title>"));
    let (status, _) = http(addr, "GET", "/api/v1/server-info", None).await;
    assert_eq!(status, 200);
    stop(tx, running, addr, "ui").await;
}

#[tokio::test]
async fn expired_intake_drafts_are_cleaned_up_at_start() {
    for (label, db) in mw_db::testing::all_backends().await {
        let dir = temp_dir(label);
        let server = Server::start_with_db(&config(dir.clone()), db.clone())
            .await
            .unwrap();
        let actor = mw_core::Actor::unrestricted("Тест");
        let key = "0f8fad5b-d9cb-469f-a165-70867728950e";
        server
            .core()
            .add_draft_attachment(&actor, key, "a.txt", b"hello".to_vec())
            .await
            .unwrap();
        db.execute_unprepared("UPDATE attachments SET created_at = '2000-01-01 00:00:00'")
            .await
            .unwrap();
        let core = server.core().clone();
        let addr = server.local_addr();
        let (tx, stopped) = tokio::sync::oneshot::channel::<()>();
        let running = tokio::spawn(server.serve(async {
            stopped.await.ok();
        }));
        let mut left = 1;
        for _ in 0..100 {
            left = core
                .list_draft_attachments(&actor, key)
                .await
                .unwrap()
                .len();
            if left == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert_eq!(left, 0, "{label}");
        stop(tx, running, addr, label).await;
    }
}

#[tokio::test]
async fn staging_left_by_an_interrupted_import_is_removed_at_start() {
    for (label, db) in mw_db::testing::all_backends().await {
        let dir = temp_dir(label);
        let staging = dir.join("attachments/.import-20261111-090000");
        std::fs::create_dir_all(staging.join("orders/1")).unwrap();
        std::fs::write(staging.join("orders/1/photo.jpg"), b"x").unwrap();
        std::fs::create_dir_all(dir.join("attachments/orders/2")).unwrap();
        std::fs::write(dir.join("attachments/orders/2/kept.jpg"), b"y").unwrap();
        let server = Server::start_with_db(&config(dir.clone()), db)
            .await
            .unwrap();
        assert!(!staging.exists(), "{label}");
        assert!(
            dir.join("attachments/orders/2/kept.jpg").exists(),
            "{label}"
        );
        drop(server);
    }
}
