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
