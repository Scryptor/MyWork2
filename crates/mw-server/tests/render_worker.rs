//! Пул процессов-рендереров (`mw_docs::RenderPool`) на настоящем `mw-server` в режиме рендерера:
//! свой код строится в отдельном процессе, бесконечный цикл обрывается пределом времени.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use mw_docs::model::{Block, BlockBody, CopyFilter};
use mw_docs::{
    DocsError, FontSources, Job, JobBytes, JobLimits, JobOutput, RenderOptions, RenderPool,
    WorkerLimits, sample_data, standard_template,
};
use mw_domain::documents::DocumentKind;
use mw_server::RENDER_WORKER_ARG;

const GOOD_CODE: &str = r#"ВОРКЕР #v("order.number")"#;
const ENDLESS_CODE: &str = "#for i in range(100000) { for j in range(100000) {} }";

const LIMITS: WorkerLimits = WorkerLimits {
    startup: Duration::from_secs(60),
    per_document: Duration::from_secs(5),
};

/// Пул общий для потоков и клонов.
const _: fn() = || {
    fn shared<T: Send + Sync + Clone>() {}
    shared::<RenderPool>();
};

fn program() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mw-server"))
}

fn pool(limits: WorkerLimits) -> RenderPool {
    RenderPool::new(program(), FontSources::default(), limits)
}

/// Наклейка из одного блока кода `code`, PDF.
fn code_job(code: &str, budget: Duration) -> Job {
    let kind = DocumentKind::Sticker;
    let mut template = standard_template(kind);
    template.blocks = vec![Block {
        copies: CopyFilter::Both,
        body: BlockBody::TypstCode {
            code: code.to_owned(),
        },
    }];
    Job {
        template,
        options: RenderOptions::default(),
        logo: None,
        docs: vec![sample_data(kind)],
        output: JobOutput::Pdf,
        limits: JobLimits {
            max_pages: 50,
            max_total_pages: 200,
            max_raster_pixels: 36_000_000.0,
            budget,
        },
    }
}

fn good_job() -> Job {
    code_job(GOOD_CODE, Duration::from_secs(20))
}

fn assert_pdf_of_one_page(result: Result<mw_docs::JobResult, DocsError>) {
    let result = result.unwrap();
    assert_eq!(result.page_count, 1);
    match &result.bytes {
        JobBytes::Pdf(pdf) => assert!(pdf.starts_with(b"%PDF")),
        other => panic!("ждали PDF: {other:?}"),
    }
}

#[test]
fn worker_renders_a_code_template() {
    let pool = pool(LIMITS);
    assert_pdf_of_one_page(pool.run(&good_job()));
}

#[test]
fn endless_code_is_stopped_and_the_next_job_works() {
    let pool = pool(LIMITS);
    // Процесс готов заранее: запуск не входит в предел документа.
    pool.warm();
    let started = Instant::now();
    let result = pool.run(&code_job(ENDLESS_CODE, Duration::from_secs(20)));
    let took = started.elapsed();
    assert_eq!(result.map(|_| ()), Err(DocsError::CodeTimeout));
    assert!(took < Duration::from_secs(10), "оборвали за {took:?}");

    assert_pdf_of_one_page(pool.run(&good_job()));
    // Убитый процесс не вернулся к простаивающим: второе задание запустило новый.
    assert_eq!(pool.started_for_tests(), 2);
}

#[test]
fn job_over_budget_is_too_slow() {
    let pool = pool(WorkerLimits {
        per_document: Duration::from_secs(30),
        ..LIMITS
    });
    pool.warm();
    let started = Instant::now();
    let result = pool.run(&code_job(ENDLESS_CODE, Duration::from_secs(1)));
    let took = started.elapsed();
    assert_eq!(result.map(|_| ()), Err(DocsError::TooSlow));
    assert!(took < Duration::from_secs(5), "оборвали за {took:?}");
}

#[test]
fn code_error_keeps_the_process() {
    let pool = pool(LIMITS);
    let result = pool.run(&code_job("#nope", Duration::from_secs(20)));
    assert!(
        matches!(result, Err(DocsError::Code(_))),
        "{:?}",
        result.map(|_| ())
    );
    assert_pdf_of_one_page(pool.run(&good_job()));
    assert_eq!(pool.started_for_tests(), 1);
}

#[test]
fn two_jobs_at_once() {
    let pool = pool(LIMITS);
    let job = good_job();
    let results: Vec<_> = std::thread::scope(|scope| {
        let runs: Vec<_> = (0..2)
            .map(|_| {
                let pool = pool.clone();
                let job = &job;
                scope.spawn(move || pool.run(job))
            })
            .collect();
        runs.into_iter().map(|run| run.join().unwrap()).collect()
    });
    for result in results {
        assert_pdf_of_one_page(result);
    }
    assert!(
        pool.started_for_tests() <= 2,
        "{}",
        pool.started_for_tests()
    );
}

#[test]
fn missing_program_is_unavailable() {
    let dir = tempfile::tempdir().unwrap();
    let pool = RenderPool::new(
        dir.path().join("нет-такой-программы"),
        FontSources::default(),
        LIMITS,
    );
    let result = pool.run(&good_job());
    assert!(
        matches!(result, Err(DocsError::WorkerUnavailable(_))),
        "{:?}",
        result.map(|_| ())
    );
}

#[test]
fn worker_exits_when_stdin_closes() {
    let mut child = Command::new(program())
        .arg(RENDER_WORKER_ARG)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    drop(child.stdin.take());
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() > deadline {
            child.kill().ok();
            child.wait().ok();
            panic!("рендерер не вышел после закрытия stdin");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(status.code(), Some(0), "{status:?}");
}
