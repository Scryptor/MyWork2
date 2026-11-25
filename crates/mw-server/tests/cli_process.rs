#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn server_info(port: u16) -> Option<String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    stream
        .write_all(
            b"GET /api/v1/server-info HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
        )
        .ok()?;
    let mut text = String::new();
    stream.read_to_string(&mut text).ok()?;
    text.contains("200 OK").then_some(text)
}

/// Запускает `mw-server` с дополнительными флагами и возвращает `setup_required` из `server-info`.
fn setup_required_with(extra: &[&str]) -> bool {
    let dir = tempfile::tempdir().unwrap();
    let port = free_port();
    let mut child = Command::new(env!("CARGO_BIN_EXE_mw-server"))
        .args([
            "--timezone",
            "UTC",
            "--log-dir",
            "none",
            "--exit-on-stdin-close",
        ])
        .args(["--listen", &format!("127.0.0.1:{port}")])
        .arg("--data-dir")
        .arg(dir.path())
        .args(extra)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    let info = loop {
        if let Some(text) = server_info(port) {
            break text;
        }
        assert!(Instant::now() < deadline, "сервер не поднялся");
        std::thread::sleep(Duration::from_millis(100));
    };
    drop(child.stdin.take());
    child.wait().unwrap();
    if info.contains("\"setup_required\":true") {
        true
    } else {
        assert!(info.contains("\"setup_required\":false"), "{info}");
        false
    }
}

#[test]
fn assume_setup_flag_marks_a_new_base_as_set_up() {
    assert!(setup_required_with(&[]), "без флага база новая");
    assert!(!setup_required_with(&["--assume-setup"]));
}

#[test]
fn the_final_error_reaches_the_log_file() {
    let dir = tempfile::tempdir().unwrap();
    let missing_ui = dir.path().join("нет-интерфейса");
    let output = Command::new(env!("CARGO_BIN_EXE_mw-server"))
        .args(["--timezone", "UTC", "--listen", "127.0.0.1:0"])
        .arg("--data-dir")
        .arg(dir.path())
        .arg("--ui-dir")
        .arg(&missing_ui)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let logs = dir.path().join("logs");
    let text: String = std::fs::read_dir(&logs)
        .unwrap()
        .map(|e| std::fs::read_to_string(e.unwrap().path()).unwrap())
        .collect();
    assert!(text.contains("ERROR"), "{text}");
    assert!(text.contains("папка интерфейса не найдена"), "{text}");
}

/// Запускает `mw-server` на папке данных и ждёт выхода; ввод — `input`, затем закрывается (сервер,
/// если всё же поднялся, по `--exit-on-stdin-close` завершается, а не зависает в тесте).
fn run_on(dir: &std::path::Path, args: &[&std::ffi::OsStr], input: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_mw-server"))
        .args([
            "--timezone",
            "UTC",
            "--log-dir",
            "none",
            "--listen",
            "127.0.0.1:0",
        ])
        .arg("--exit-on-stdin-close")
        .arg("--data-dir")
        .arg(dir)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn serve_and_import_refuse_a_data_folder_in_use_and_create_admin_does_not() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let old = tempfile::tempdir().unwrap();
    let held = mw_server::lock::acquire(&data).unwrap();
    let commands: [&[&std::ffi::OsStr]; 2] = [
        &["serve".as_ref()],
        &["import-legacy".as_ref(), old.path().as_os_str()],
    ];
    for args in commands {
        let output = run_on(&data, args, "");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{args:?}: {stderr}");
        assert!(
            stderr.contains("уже использует другая копия программы"),
            "{args:?}: {stderr}"
        );
        assert!(
            !data.join("mywork.db").exists(),
            "{args:?}: база не открыта"
        );
    }
    // Администратора можно создать и при работающей программе: это одна короткая запись в базу.
    let output = run_on(
        &data,
        &["create-admin".as_ref(), "--login".as_ref(), "boss".as_ref()],
        "test-password-1\n",
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    drop(held);
    let output = run_on(
        &data,
        &["import-legacy".as_ref(), old.path().as_os_str()],
        "",
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("другая копия"), "замок отпущен: {stderr}");
}

async fn logins(data: &std::path::Path) -> Vec<String> {
    use mw_db::entity::user;
    use sea_orm::{EntityTrait, QueryOrder};
    let db = mw_db::connect(&mw_db::sqlite_url(&data.join("mywork.db")))
        .await
        .unwrap();
    let rows = user::Entity::find()
        .order_by_asc(user::Column::Login)
        .all(&db)
        .await
        .unwrap();
    let _ = db.close().await;
    rows.into_iter().map(|u| u.login).collect()
}

fn create_admin_on(data: &std::path::Path, login: &str) {
    let output = run_on(
        data,
        &["create-admin".as_ref(), "--login".as_ref(), login.as_ref()],
        "test-password-1\n",
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn backup_and_restore_commands_bring_the_data_back() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    create_admin_on(&data, "first");
    let output = run_on(&data, &["backup".as_ref()], "");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let path = stdout
        .lines()
        .find_map(|l| l.strip_prefix("Копия сохранена: "))
        .unwrap_or_else(|| panic!("{stdout}"))
        .trim()
        .to_string();
    let copy = std::path::PathBuf::from(&path);
    assert!(copy.is_absolute(), "{path}");
    assert!(copy.join("mywork-backup.json").is_file(), "{path}");

    create_admin_on(&data, "second");
    assert_eq!(logins(&data).await, ["first", "second"]);
    let output = run_on(&data, &["restore".as_ref(), copy.as_os_str()], "");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{stdout} {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("Данные восстановлены из копии от "),
        "{stdout}"
    );
    assert!(stdout.contains("Прежние данные: "), "{stdout}");
    assert_eq!(logins(&data).await, ["first"]);
}

#[test]
fn restore_refuses_a_folder_without_a_copy() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let empty = dir.path().join("пусто");
    std::fs::create_dir_all(&empty).unwrap();
    let output = run_on(&data, &["restore".as_ref(), empty.as_os_str()], "");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_ne!(output.status.code(), Some(0));
    assert!(stderr.contains("Такой копии нет"), "{stderr}");
}
