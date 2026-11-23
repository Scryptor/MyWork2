//! Пул с поддельными рендерерами — сценариями оболочки: падение посреди задания, запуск дольше
//! предела, сборка файла дольше предела документа, замена убитого процесса. Настоящий рендерер
//! проверяют тесты `mw-server` (`tests/render_worker.rs`).
//!
//! Все случаи — в одном тесте, и все сценарии пишутся до первого запуска: сценарий, который пишет
//! один поток, пока другой запускает процесс, может не запуститься («Text file busy»).
#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use mw_docs::{
    DocsError, FontSources, Job, JobBytes, JobLimits, JobOutput, RenderOptions, RenderPool,
    WorkerLimits, sample_data, standard_template,
};
use mw_domain::documents::DocumentKind;

fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

/// Команда оболочки, которая пишет кадр `bytes` (длина little-endian и байты): `printf` с
/// восьмеричными escape-последовательностями длины. В `bytes` нет `'` и `\`.
fn frame(bytes: &str) -> String {
    let len: String = u32::try_from(bytes.len())
        .unwrap()
        .to_le_bytes()
        .iter()
        .map(|b| format!("\\{b:03o}"))
        .collect();
    format!("printf '{len}{}'", bytes.replace('%', "%%"))
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

fn pool(program: PathBuf, startup: Duration, per_document: Duration) -> RenderPool {
    let limits = WorkerLimits {
        startup,
        per_document,
    };
    RenderPool::new(program, FontSources::default(), limits)
}

#[test]
fn fake_workers() {
    let dir = tempfile::tempdir().unwrap();
    let ready = frame(r#""Ready""#);
    // Выход посреди задания.
    let exits = script(dir.path(), "exits.sh", &format!("{ready}; sleep 1"));
    // Не отвечает: `exec` — чтобы убитый процесс не оставил `sleep` с открытым stdout.
    let silent = script(dir.path(), "silent.sh", "exec sleep 30");
    // Готов, но задание не строит.
    let stuck = script(dir.path(), "stuck.sh", &format!("{ready}; exec sleep 30"));
    // Документ готов сразу, а файл собирается дольше предела документа.
    let slow_file = script(
        dir.path(),
        "slow-file.sh",
        &format!(
            "{ready}; sleep 0.2; {}; sleep 1.5; {}; {}; exec sleep 30",
            frame(r#"{"Compiled":{"index":0}}"#),
            frame(
                r#"{"Done":{"page_count":1,"page_size_mm":[58.0,40.0],"format":"Pdf","frames":1}}"#
            ),
            frame("%PDF-fake"),
        ),
    );
    let long = Duration::from_secs(60);
    let second = Duration::from_secs(1);

    // Упал посреди задания: `WorkerCrashed`, вместо него заранее запущен новый.
    let crashing = pool(exits, long, long);
    let result = crashing.run(&job());
    assert_eq!(result.map(|_| ()), Err(DocsError::WorkerCrashed));
    crashing.wait_rewarm_for_tests();
    assert_eq!(crashing.started_for_tests(), 2);

    // Не стал готов за `startup`: `WorkerUnavailable` быстро; это не убитый посреди задания процесс,
    // замены нет.
    let started = Instant::now();
    let unready = pool(silent, Duration::from_millis(500), long);
    let result = unready.run(&job());
    let took = started.elapsed();
    assert!(
        matches!(result, Err(DocsError::WorkerUnavailable(_))),
        "{:?}",
        result.map(|_| ())
    );
    assert!(took < Duration::from_secs(10), "{took:?}");
    unready.wait_rewarm_for_tests();
    assert_eq!(unready.started_for_tests(), 1);

    // После последнего `Compiled` действует только общий срок задания: сборка файла дольше
    // `per_document` — не `CodeTimeout`.
    let result = pool(slow_file, long, second).run(&job());
    match result {
        Ok(result) => {
            assert_eq!(result.page_count, 1);
            assert!(
                matches!(&result.bytes, JobBytes::Pdf(pdf) if pdf == b"%PDF-fake"),
                "{:?}",
                result.bytes
            );
        }
        Err(error) => panic!("ждали PDF: {error}"),
    }

    // Убитый по сроку процесс заменяется заранее, и следующее задание берёт замену, а не
    // запускает свой процесс.
    let killing = pool(stuck.clone(), long, Duration::from_millis(300));
    assert_eq!(killing.run(&job()).map(|_| ()), Err(DocsError::CodeTimeout));
    killing.wait_rewarm_for_tests();
    assert_eq!(killing.started_for_tests(), 2);
    assert_eq!(killing.run(&job()).map(|_| ()), Err(DocsError::CodeTimeout));
    assert_eq!(
        killing.started_for_tests(),
        2,
        "задание запустило свой процесс"
    );
    killing.wait_rewarm_for_tests();
    assert_eq!(killing.started_for_tests(), 3);

    // Есть простаивающий — замены нет.
    let spare = pool(stuck, long, Duration::from_millis(300));
    spare.warm();
    spare.warm();
    assert_eq!(spare.run(&job()).map(|_| ()), Err(DocsError::CodeTimeout));
    spare.wait_rewarm_for_tests();
    assert_eq!(spare.started_for_tests(), 2);
}
