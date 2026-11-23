//! Задание рендера: один или несколько документов, пределы страниц и пикселей, бюджет времени.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use mw_docs::model::{Align, Block, BlockBody, CopyFilter, Template};
use mw_docs::{
    DocsError, Job, JobBytes, JobLimits, JobOutput, JobResult, RenderOptions, render, run_job,
    sample_data, standard_template,
};
use mw_domain::documents::DocumentKind;

fn sticker() -> Template {
    standard_template(DocumentKind::Sticker)
}

fn job(template: Template, docs: usize, output: JobOutput) -> Job {
    Job {
        template,
        options: RenderOptions::default(),
        logo: None,
        docs: vec![sample_data(DocumentKind::Sticker); docs],
        output,
        limits: JobLimits {
            max_pages: 50,
            max_total_pages: 200,
            max_raster_pixels: 36_000_000.0,
            budget: Duration::from_secs(60),
        },
    }
}

fn run(job: &Job) -> Result<JobResult, DocsError> {
    run_job(job, &mut |_| {})
}

fn pdf(result: &JobResult) -> &[u8] {
    match &result.bytes {
        JobBytes::Pdf(pdf) => pdf,
        other => panic!("ждали PDF: {other:?}"),
    }
}

fn pages(result: &JobResult) -> &[Vec<u8>] {
    match &result.bytes {
        JobBytes::Png(pages) => pages,
        other => panic!("ждали PNG: {other:?}"),
    }
}

/// A grey PNG of `width` × `height` pixels.
fn png(width: u32, height: u32) -> Arc<[u8]> {
    let image = image::RgbImage::from_pixel(width, height, image::Rgb([40, 90, 160]));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    Arc::from(bytes.into_inner())
}

#[test]
fn one_document_job_matches_render() {
    let job = job(sticker(), 1, JobOutput::Pdf);
    let rendered = render(&job.template, &job.docs[0], &job.options).unwrap();
    let result = run(&job).unwrap();
    assert_eq!(result.page_count, rendered.page_count());
    assert_eq!(result.page_size_mm, rendered.page_size_mm());
    assert!(pdf(&result).starts_with(b"%PDF"));
}

#[test]
fn batch_job_merges_and_reports_each() {
    let job = job(sticker(), 3, JobOutput::Pdf);
    let mut compiled = Vec::new();
    let result = run_job(&job, &mut |i| compiled.push(i)).unwrap();
    assert_eq!(result.page_count, 3);
    assert_eq!(compiled, vec![0, 1, 2]);
    assert!(pdf(&result).starts_with(b"%PDF"));
}

#[test]
fn png_job_gives_a_page_each() {
    let result = run(&job(sticker(), 2, JobOutput::Png { dpi: 72 })).unwrap();
    let pages = pages(&result);
    assert_eq!((pages.len(), result.page_count), (2, 2));
    assert!(pages.iter().all(|p| p.starts_with(b"\x89PNG")));
}

#[test]
fn page_limits_hold() {
    let mut one = job(sticker(), 1, JobOutput::Pdf);
    one.limits.max_pages = 0;
    let r = run(&one);
    assert!(matches!(r, Err(DocsError::TooManyPages(1))), "{r:?}");

    let mut two = job(sticker(), 2, JobOutput::Pdf);
    two.limits.max_total_pages = 1;
    let mut compiled = Vec::new();
    let r = run_job(&two, &mut |i| compiled.push(i));
    assert!(matches!(r, Err(DocsError::TooManyPages(2))), "{r:?}");
    assert_eq!(compiled, vec![0], "второй документ не прошёл предел");
}

/// Растр проверяется до построения: PDF того же задания предел пикселей не трогает.
#[test]
fn raster_limit_holds() {
    let mut raster = job(sticker(), 1, JobOutput::Png { dpi: 72 });
    raster.limits.max_raster_pixels = 1.0;
    let r = run(&raster);
    assert!(matches!(r, Err(DocsError::TooManyPixels)), "{r:?}");
    raster.output = JobOutput::Pdf;
    run(&raster).unwrap();
}

#[test]
fn budget_stops_between_documents() {
    let mut job = job(sticker(), 2, JobOutput::Pdf);
    job.limits.budget = Duration::ZERO;
    let mut compiled = Vec::new();
    let r = run_job(&job, &mut |i| compiled.push(i));
    assert!(matches!(r, Err(DocsError::TooSlow)), "{r:?}");
    assert!(compiled.is_empty(), "{compiled:?}");
}

#[test]
fn job_logo_reaches_every_document() {
    let mut template = sticker();
    template.blocks = vec![Block {
        copies: CopyFilter::Both,
        body: BlockBody::Logo {
            width_mm: 30.0,
            height_mm: 15.0,
            align: Align::Center,
        },
    }];
    let mut with_logo = job(template.clone(), 2, JobOutput::Pdf);
    with_logo.logo = Some(png(40, 20));
    let image = b"/Image";
    let printed = run(&with_logo).unwrap();
    assert!(
        pdf(&printed).windows(image.len()).any(|w| w == image),
        "в PDF нет картинки"
    );

    // Логотип задания — на каждом листе; свой логотип документа не читается.
    with_logo.output = JobOutput::Png { dpi: 72 };
    let mut without = job(template, 2, JobOutput::Png { dpi: 72 });
    for doc in &mut without.docs {
        doc.logo = Some(png(40, 20));
    }
    let with_logo = run(&with_logo).unwrap();
    let without = run(&without).unwrap();
    let (with_logo, without) = (pages(&with_logo), pages(&without));
    assert_eq!((with_logo.len(), without.len()), (2, 2));
    for (i, (a, b)) in with_logo.iter().zip(without).enumerate() {
        assert_ne!(a, b, "лист {i}: логотипа нет");
    }
}

#[test]
fn job_round_trips_as_json() {
    let mut job = job(sticker(), 2, JobOutput::Png { dpi: 203 });
    job.options = RenderOptions {
        label_mm: Some((58.0, 40.0)),
        offset_mm: (0.5, -1.25),
        dpi: Some(203),
    };
    job.docs[1].blank = true;
    job.docs[1].barcode = None;
    job.limits.budget = Duration::from_millis(25_500);
    let back: Job = serde_json::from_str(&serde_json::to_string(&job).unwrap()).unwrap();
    assert_eq!(back, job);

    // Логотип в JSON не входит: по проводу он идёт отдельно.
    job.logo = Some(png(4, 4));
    job.docs[0].logo = Some(png(4, 4));
    let back: Job = serde_json::from_str(&serde_json::to_string(&job).unwrap()).unwrap();
    assert!(
        back.logo.is_none() && back.docs[0].logo.is_none(),
        "{back:?}"
    );
}
