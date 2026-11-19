#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn server_exits_when_stdin_is_closed() {
    let dir = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_mw-server"))
        .args([
            "--listen",
            "127.0.0.1:0",
            "--timezone",
            "UTC",
            "--log-dir",
            "none",
        ])
        .arg("--data-dir")
        .arg(dir.path())
        .arg("--exit-on-stdin-close")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Даём серверу подняться, затем закрываем stdin: он обязан завершиться сам.
    std::thread::sleep(Duration::from_millis(1500));
    assert!(
        child.try_wait().unwrap().is_none(),
        "сервер вышел раньше времени"
    );
    drop(child.stdin.take());
    let deadline = Instant::now() + Duration::from_secs(15);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() > deadline {
            child.kill().ok();
            let mut err = String::new();
            child.stderr.take().unwrap().read_to_string(&mut err).ok();
            panic!("сервер не завершился после закрытия stdin: {err}");
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    assert!(status.success(), "{status:?}");
}
