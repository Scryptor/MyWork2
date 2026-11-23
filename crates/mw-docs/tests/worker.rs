//! Процесс-рендерер со стороны рендерера: [`serve`] читает кадры родителя и отвечает кадрами.
//!
//! Кадры здесь собираются и разбираются по описанию протокола (`src/wire.rs`): кадр — `u32` длины
//! (little-endian) и байты; сообщение — кадр JSON и объявленные в нём двоичные кадры.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::io::{self, Read};
use std::sync::Arc;
use std::time::Duration;

use mw_docs::model::{Align, Block, BlockBody, CodeDiagnostic, CodePlace, CopyFilter};
use mw_docs::{
    DocsError, FontSources, Job, JobBytes, JobLimits, JobOutput, JobResult, RenderOptions,
    error_through_wire_for_tests, run_job, sample_data, serve, standard_template,
};
use mw_domain::documents::DocumentKind;
use serde_json::{Value, json};

fn frame(bytes: &[u8]) -> Vec<u8> {
    let mut out = u32::try_from(bytes.len()).unwrap().to_le_bytes().to_vec();
    out.extend_from_slice(bytes);
    out
}

fn message(json: &Value) -> Vec<u8> {
    frame(&serde_json::to_vec(json).unwrap())
}

fn hello() -> Vec<u8> {
    message(&json!({ "Hello": { "fonts": FontSources::default() } }))
}

/// `Run` с заданием без логотипа; логотип идёт за ним одним двоичным кадром.
fn run(job: &Job) -> Vec<u8> {
    let mut out = message(&json!({
        "Run": { "job": job, "logo": job.logo.is_some(), "deadline_ms": 25_000 }
    }));
    if let Some(logo) = &job.logo {
        out.extend(frame(logo));
    }
    out
}

/// Сообщение рендерера с объявленными в нём двоичными кадрами.
#[derive(Debug)]
struct Reply {
    message: Value,
    frames: Vec<Vec<u8>>,
}

fn take_frame(input: &mut &[u8]) -> Vec<u8> {
    let (len, rest) = input.split_at(4);
    let len = u32::from_le_bytes(len.try_into().unwrap()) as usize;
    let (bytes, rest) = rest.split_at(len);
    *input = rest;
    bytes.to_vec()
}

/// Все кадры подряд, без разбора.
fn frames(mut input: &[u8]) -> Vec<Vec<u8>> {
    let mut frames = Vec::new();
    while !input.is_empty() {
        frames.push(take_frame(&mut input));
    }
    frames
}

fn replies(mut output: &[u8]) -> Vec<Reply> {
    let mut replies = Vec::new();
    while !output.is_empty() {
        let message: Value = serde_json::from_slice(&take_frame(&mut output)).unwrap();
        let declared = message["Done"]["frames"].as_u64().unwrap_or(0);
        let frames = (0..declared).map(|_| take_frame(&mut output)).collect();
        replies.push(Reply { message, frames });
    }
    replies
}

/// Ответы на `input`, который кончается между сообщениями.
fn serve_all(input: &[u8]) -> Vec<Reply> {
    let mut output = Vec::new();
    serve(input, &mut output).unwrap();
    replies(&output)
}

fn messages(replies: &[Reply]) -> Vec<&Value> {
    replies.iter().map(|r| &r.message).collect()
}

fn refused(input: impl Read) -> io::ErrorKind {
    serve(input, io::sink()).unwrap_err().kind()
}

fn sticker_job(docs: usize, output: JobOutput) -> Job {
    let kind = DocumentKind::Sticker;
    Job {
        template: standard_template(kind),
        options: RenderOptions::default(),
        logo: None,
        docs: vec![sample_data(kind); docs],
        output,
        limits: JobLimits {
            max_pages: 50,
            max_total_pages: 200,
            max_raster_pixels: 36_000_000.0,
            budget: Duration::from_secs(60),
        },
    }
}

fn in_process(job: &Job) -> JobResult {
    run_job(job, &mut |_| {}).unwrap()
}

fn png_pages(result: JobResult) -> Vec<Vec<u8>> {
    match result.bytes {
        JobBytes::Png(pages) => pages,
        other => panic!("ждали PNG: {other:?}"),
    }
}

/// Серый PNG `width` × `height` точек.
fn png(width: u32, height: u32) -> Arc<[u8]> {
    let image = image::RgbImage::from_pixel(width, height, image::Rgb([40, 90, 160]));
    let mut bytes = io::Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    Arc::from(bytes.into_inner())
}

fn compiled(index: usize) -> Value {
    json!({ "Compiled": { "index": index } })
}

#[test]
fn hello_then_ready() {
    let replies = serve_all(&hello());
    assert_eq!(messages(&replies), [&json!("Ready")]);
}

#[test]
fn job_reports_each_document_then_done() {
    let job = sticker_job(2, JobOutput::Pdf);
    let replies = serve_all(&[hello(), run(&job)].concat());
    assert_eq!(
        messages(&replies)[..3],
        [&json!("Ready"), &compiled(0), &compiled(1)]
    );
    assert_eq!(replies.len(), 4, "{replies:?}");
    let done = &replies[3];
    let fields = &done.message["Done"];
    assert_eq!(
        (&fields["format"], &fields["page_count"], &fields["frames"]),
        (&json!("Pdf"), &json!(2), &json!(1)),
        "{fields}"
    );
    let size: (f64, f64) = serde_json::from_value(fields["page_size_mm"].clone()).unwrap();
    assert_eq!(size, in_process(&job).page_size_mm);
    assert_eq!(done.frames.len(), 1);
    assert!(done.frames[0].starts_with(b"%PDF"));
}

#[test]
fn png_job_sends_a_frame_per_page() {
    let job = sticker_job(2, JobOutput::Png { dpi: 72 });
    let replies = serve_all(&[hello(), run(&job)].concat());
    let done = replies.last().unwrap();
    let fields = &done.message["Done"];
    assert_eq!(
        (&fields["format"], &fields["page_count"], &fields["frames"]),
        (&json!("Png"), &json!(2), &json!(2)),
        "{fields}"
    );
    assert!(done.frames.iter().all(|p| p.starts_with(b"\x89PNG")));
    assert_eq!(done.frames, png_pages(in_process(&job)));
}

#[test]
fn code_errors_cross_the_pipe() {
    let mut job = sticker_job(1, JobOutput::Pdf);
    job.template.blocks = vec![Block {
        copies: CopyFilter::Both,
        body: BlockBody::TypstCode {
            code: "#nope".to_owned(),
        },
    }];
    let replies = serve_all(&[hello(), run(&job)].concat());
    assert_eq!(replies.len(), 2, "{replies:?}");
    let error = &replies[1].message["Failed"]["error"];
    let restored = DocsError::Code(serde_json::from_value(error["Code"].clone()).unwrap());

    let DocsError::Code(diagnostics) = &restored else {
        unreachable!()
    };
    let place = diagnostics[0].place.as_ref().expect("место в коде");
    assert_eq!((place.block.as_slice(), place.line), (&[0][..], 1));
    assert_eq!(Err(restored), run_job(&job, &mut |_| {}).map(|_| ()));
}

/// Номер варианта. Новый вариант `DocsError` не соберётся здесь, пока не получит номер (следующий
/// после [`LAST_VARIANT`]) и образец в [`every_error`].
fn variant(error: &DocsError) -> usize {
    match error {
        DocsError::Compile(_) => 0,
        DocsError::Code(_) => 1,
        DocsError::Pdf(_) => 2,
        DocsError::Png(_) => 3,
        DocsError::BadFileName(_) => 4,
        DocsError::Dpi(_) => 5,
        DocsError::TooManyPages(_) => 6,
        DocsError::TooManyPixels => 7,
        DocsError::TooSlow => 8,
    }
}
const LAST_VARIANT: usize = 8;

/// По ошибке каждого варианта.
fn every_error() -> Vec<DocsError> {
    vec![
        DocsError::Compile(vec!["unknown variable: x".to_owned(), "второй".to_owned()]),
        DocsError::Code(vec![
            CodeDiagnostic {
                place: Some(CodePlace {
                    block: vec![2, 1, 0],
                    line: 3,
                    column: 7,
                }),
                message: "expected expression".to_owned(),
                hints: vec!["try `#{ }`".to_owned()],
            },
            CodeDiagnostic {
                place: None,
                message: "без места".to_owned(),
                hints: vec![],
            },
        ]),
        DocsError::Pdf(vec!["a".to_owned(), "б".to_owned()]),
        DocsError::Png("encoder".to_owned()),
        DocsError::BadFileName("../x".to_owned()),
        DocsError::Dpi(0),
        DocsError::TooManyPages(201),
        DocsError::TooManyPixels,
        DocsError::TooSlow,
    ]
}

#[test]
fn every_error_round_trips() {
    let numbers: BTreeSet<usize> = every_error().iter().map(variant).collect();
    assert_eq!(
        numbers,
        (0..=LAST_VARIANT).collect(),
        "образец каждого варианта"
    );
    for (error, expected) in every_error().into_iter().zip(every_error()) {
        assert_eq!(error_through_wire_for_tests(error).unwrap(), expected);
    }
}

#[test]
fn logo_goes_once() {
    let mut job = sticker_job(2, JobOutput::Png { dpi: 72 });
    job.template.blocks = vec![Block {
        copies: CopyFilter::Both,
        body: BlockBody::Logo {
            width_mm: 30.0,
            height_mm: 15.0,
            align: Align::Center,
        },
    }];
    job.logo = Some(png(40, 20));
    let input = [hello(), run(&job)].concat();
    let sent = frames(&input);
    assert_eq!(sent.len(), 3, "Hello, Run и один кадр логотипа");
    assert_eq!(sent[2], job.logo.as_deref().unwrap());

    let replies = serve_all(&input);
    let done = replies.last().unwrap();
    assert!(done.message.get("Done").is_some(), "{replies:?}");
    // Логотип дошёл до обоих документов: листы те же, что в процессе с логотипом, и не те, что без него.
    assert_eq!(done.frames, png_pages(in_process(&job)));
    job.logo = None;
    let without = png_pages(in_process(&job));
    for (i, (with_logo, without)) in done.frames.iter().zip(&without).enumerate() {
        assert_ne!(with_logo, without, "лист {i}: логотипа нет");
    }
}

#[test]
fn eof_between_messages_ends_cleanly() {
    assert!(serve_all(&[]).is_empty());
    assert_eq!(messages(&serve_all(&hello())), [&json!("Ready")]);
    let job = sticker_job(1, JobOutput::Pdf);
    let replies = serve_all(&[hello(), run(&job), run(&job)].concat());
    let done: Vec<_> = replies
        .iter()
        .filter(|r| r.message.get("Done").is_some())
        .collect();
    assert_eq!(done.len(), 2, "{replies:?}");
}

#[test]
fn broken_input_is_refused() {
    let hello = hello();
    let job = sticker_job(1, JobOutput::Pdf);
    let whole = [hello.clone(), run(&job)].concat();
    // Обрыв посреди кадра: в длине, в байтах.
    assert_eq!(refused(&hello[..2]), io::ErrorKind::UnexpectedEof);
    assert_eq!(
        refused(&hello[..hello.len() - 1]),
        io::ErrorKind::UnexpectedEof
    );
    assert_eq!(
        refused(&whole[..whole.len() - 3]),
        io::ErrorKind::UnexpectedEof
    );
    // Логотип объявлен, но не пришёл.
    let mut with_logo = job.clone();
    with_logo.logo = Some(png(4, 4));
    let run_with_logo = run(&with_logo);
    let header = frames(&run_with_logo).swap_remove(0);
    let cut = [hello.clone(), frame(&header)].concat();
    assert_eq!(refused(cut.as_slice()), io::ErrorKind::UnexpectedEof);

    // Длина больше 256 МиБ — отказ сразу: бесконечный вход за ней не читается.
    for len in [u32::MAX, (256 << 20) + 1] {
        let len_bytes = len.to_le_bytes();
        let endless = hello.as_slice().chain(&len_bytes[..]).chain(io::repeat(0));
        assert_eq!(refused(endless), io::ErrorKind::InvalidData, "{len}");
    }

    // Не тот порядок, не то сообщение.
    assert_eq!(refused(run(&job).as_slice()), io::ErrorKind::InvalidData);
    assert_eq!(
        refused([hello.clone(), hello.clone()].concat().as_slice()),
        io::ErrorKind::InvalidData
    );
    assert_eq!(
        refused(frame(b"{\"Hello\":").as_slice()),
        io::ErrorKind::InvalidData
    );
    assert_eq!(
        refused(message(&json!("Ready")).as_slice()),
        io::ErrorKind::InvalidData
    );
}
