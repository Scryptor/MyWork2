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
use mw_domain::pages::PageSelection;
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
        pages: None,
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

/// Выбор страниц доходит до настоящего процесса-рендерера: из двух страниц в PDF одна.
#[test]
fn chosen_pages_reach_the_worker() {
    let pool = pool(LIMITS);
    let mut job = good_job();
    job.docs.push(sample_data(DocumentKind::Sticker));
    assert_eq!(pool.run(&job).unwrap().page_count, 2);
    job.pages = Some(PageSelection::parse("2").unwrap());
    assert_pdf_of_one_page(pool.run(&job));
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

    // Убитый процесс не вернулся к простаивающим: вместо него заранее запущен новый, и следующее
    // задание берёт его, а не запускает свой.
    pool.wait_rewarm_for_tests();
    assert_eq!(pool.started_for_tests(), 2);
    assert_pdf_of_one_page(pool.run(&good_job()));
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

/// Запрос к серверу по HTTP/1.1 с телом JSON: статус и тело ответа.
async fn http(addr: std::net::SocketAddr, path: &str, body: &serde_json::Value) -> (u16, String) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let body = body.to_string();
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
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

/// Сервер с процессом-рендерером: бесконечный цикл своего кода в превью обрывается пределом
/// документа ядра — 422 `code_timeout` раньше `REQUEST_TIMEOUT`; следующее превью строится. Время
/// меряется на готовом процессе: холодный запуск со шрифтами системы в отладочной сборке (Windows CI)
/// в предел не входит.
#[tokio::test]
async fn http_preview_with_endless_code_times_out() {
    for (label, db) in mw_db::testing::all_backends().await {
        let dir = tempfile::tempdir().unwrap();
        let config = mw_server::Config {
            listen: "127.0.0.1:0".parse().unwrap(),
            data_dir: dir.path().to_path_buf(),
            database_url: None,
            timezone: Some("UTC".into()),
            ui_dir: None,
            render_worker: Some(program()),
        };
        let server = mw_server::Server::start_with_db(&config, db).await.unwrap();
        let addr = server.local_addr();
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        let running = tokio::spawn(server.serve(async {
            stopped.await.ok();
        }));
        let preview = "/api/v1/document-templates/sticker/preview";

        // Рабочий код: после ответа процесс-рендерер готов.
        let good = good_job().template;
        let (status, body) = http(addr, preview, &serde_json::json!({ "template": good })).await;
        assert_eq!(status, 200, "{label}: {body}");

        let endless = code_job(ENDLESS_CODE, Duration::from_secs(20)).template;
        let started = Instant::now();
        let (status, body) = http(addr, preview, &serde_json::json!({ "template": endless })).await;
        let took = started.elapsed();
        assert_eq!(status, 422, "{label}: {body}");
        assert!(body.contains("code_timeout"), "{label}: {body}");
        assert!(took < Duration::from_secs(15), "{label}: ответ за {took:?}");

        let standard = standard_template(DocumentKind::Sticker);
        let (status, body) =
            http(addr, preview, &serde_json::json!({ "template": standard })).await;
        assert_eq!(status, 200, "{label}: {body}");

        stop.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(10), running)
            .await
            .expect("сервер не остановился")
            .unwrap()
            .unwrap();
    }
}
