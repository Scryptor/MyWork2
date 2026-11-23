//! Пул с поддельными рендерерами — сценариями оболочки: падение посреди задания и запуск дольше
//! предела. Настоящий рендерер проверяют тесты `mw-server` (`tests/render_worker.rs`).
//!
//! Оба случая — в одном тесте: сценарий, который пишет один поток, пока другой запускает процесс,
//! может не запуститься («Text file busy»).
#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use mw_docs::{
    DocsError, FontSources, Job, JobLimits, JobOutput, RenderOptions, RenderPool, WorkerLimits,
    sample_data, standard_template,
};
use mw_domain::documents::DocumentKind;

fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn job() -> Job {
    let kind = DocumentKind::Sticker;
    Job {
        template: standard_template(kind),
        options: RenderOptions::default(),
        logo: None,
        docs: vec![sample_data(kind)],
        output: JobOutput::Pdf,
        limits: JobLimits {
            max_pages: 50,
            max_total_pages: 200,
            max_raster_pixels: 36_000_000.0,
            budget: Duration::from_secs(20),
        },
    }
}

fn pool(program: PathBuf, startup: Duration) -> RenderPool {
    let limits = WorkerLimits {
        startup,
        per_document: Duration::from_secs(30),
    };
    RenderPool::new(program, FontSources::default(), limits)
}

#[test]
fn broken_workers_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    // Кадр `"Ready"` (7 байт), затем выход посреди задания.
    let exits = script(
        dir.path(),
        "exits.sh",
        r#"printf '\007\000\000\000"Ready"'; sleep 1"#,
    );
    // Не отвечает: `exec` — чтобы убитый процесс не оставил `sleep` с открытым stdout.
    let silent = script(dir.path(), "silent.sh", "exec sleep 30");

    let result = pool(exits, Duration::from_secs(60)).run(&job());
    assert_eq!(result.map(|_| ()), Err(DocsError::WorkerCrashed));

    let started = Instant::now();
    let result = pool(silent, Duration::from_millis(500)).run(&job());
    let took = started.elapsed();
    assert!(
        matches!(result, Err(DocsError::WorkerUnavailable(_))),
        "{:?}",
        result.map(|_| ())
    );
    assert!(took < Duration::from_secs(10), "{took:?}");
}
