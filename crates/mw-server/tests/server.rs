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
