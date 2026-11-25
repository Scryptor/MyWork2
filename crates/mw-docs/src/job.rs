//! Задание рендера: документы одного шаблона подряд в одном файле, с пределами страниц, пикселей и
//! времени. Задание — данные: его можно передать в JSON отдельному процессу, логотип идёт рядом
//! байтами.

use std::sync::Arc;
use std::time::{Duration, Instant};

use mw_domain::pages::PageSelection;
use serde::{Deserialize, Serialize};

use crate::fonts::{FontSet, shared};
use crate::model::Template;
use crate::render::{DocsError, merge};
use crate::typst_gen::{DocumentData, RenderOptions, render_with_logo};

/// Что построить: документы `docs` по шаблону `template`, подряд в одном файле.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Job {
    pub template: Template,
    pub options: RenderOptions,
    /// Логотип фирмы, один на всё задание; `DocumentData::logo` у `docs` не читается.
    #[serde(skip)]
    pub logo: Option<Arc<[u8]>>,
    /// Не пусто (проверяет вызывающий).
    pub docs: Vec<DocumentData>,
    pub output: JobOutput,
    pub limits: JobLimits,
    /// Какие страницы файла выгрузить; `None` — все. Поле появилось позже остальных: кадр задания
    /// без него читается как «все страницы».
    #[serde(default)]
    pub pages: Option<PageSelection>,
}

/// В каком виде отдать результат.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobOutput {
    Pdf,
    /// Страница PNG на каждый лист, `dpi` точек на дюйм.
    Png {
        dpi: u32,
    },
}

/// Пределы задания.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct JobLimits {
    /// Страниц в одном документе не больше.
    pub max_pages: usize,
    /// Страниц во всём задании не больше.
    pub max_total_pages: usize,
    /// Пикселей на страницу растра не больше (у [`JobOutput::Png`]).
    pub max_raster_pixels: f64,
    /// Сколько может идти компиляция документов от начала задания; проверяется после каждого.
    pub budget: Duration,
}

/// Построенный файл.
#[derive(Debug)]
pub struct JobResult {
    /// Страниц в файле: при выборе страниц — выбранных.
    pub page_count: usize,
    /// Размер первой страницы файла, мм (ширина, высота).
    pub page_size_mm: (f64, f64),
    pub bytes: JobBytes,
}

/// PDF или PNG по странице.
pub enum JobBytes {
    Pdf(Vec<u8>),
    Png(Vec<Vec<u8>>),
}

/// В отладочном выводе — размеры, а не байты.
impl std::fmt::Debug for JobBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pdf(pdf) => write!(f, "Pdf({} bytes)", pdf.len()),
            Self::Png(pages) => {
                let sizes: Vec<usize> = pages.iter().map(Vec::len).collect();
                write!(f, "Png({sizes:?} bytes)")
            }
        }
    }
}

/// Выполняет `job` на общем наборе шрифтов ([`run_job_with_fonts`]).
pub fn run_job(job: &Job, on_compiled: &mut dyn FnMut(usize)) -> Result<JobResult, DocsError> {
    run_job_with_fonts(job, shared(), on_compiled)
}

/// Выполняет `job` на наборе шрифтов `fonts`. Документы компилируются по очереди; после каждого
/// проверяются пределы: страниц в нём больше `max_pages` — [`DocsError::TooManyPages`] с его
/// страницами, всего больше `max_total_pages` — то же со всеми; с начала задания прошло больше
/// `budget` — [`DocsError::TooSlow`]. Иначе зовётся `on_compiled` с номером документа. Один
/// документ берётся как есть, несколько склеиваются ([`merge`]). С выбором `pages` в файл идут
/// только эти страницы; выбранной страницы нет в документе — [`DocsError::NoPages`]. Растр сначала
/// проверяется по первой странице файла против `max_raster_pixels` ([`DocsError::TooManyPixels`]).
pub fn run_job_with_fonts(
    job: &Job,
    fonts: &FontSet,
    on_compiled: &mut dyn FnMut(usize),
) -> Result<JobResult, DocsError> {
    let started = Instant::now();
    let limits = &job.limits;
    let mut docs = Vec::with_capacity(job.docs.len());
    let mut total = 0;
    for (i, data) in job.docs.iter().enumerate() {
        let doc = render_with_logo(
            &job.template,
            data,
            job.logo.as_deref(),
            &job.options,
            fonts,
        )?;
        let pages = doc.page_count();
        total += pages;
        if pages > limits.max_pages {
            return Err(DocsError::TooManyPages(pages));
        }
        if total > limits.max_total_pages {
            return Err(DocsError::TooManyPages(total));
        }
        if started.elapsed() > limits.budget {
            return Err(DocsError::TooSlow);
        }
        on_compiled(i);
        docs.push(doc);
    }
    let doc = match <[_; 1]>::try_from(docs) {
        Ok([one]) => one,
        Err(docs) => merge(docs),
    };
    let pages = job.pages.as_ref();
    doc.check_pages(pages)?;
    let first = pages
        .and_then(|p| p.as_slice().first())
        .map_or(1, |&page| usize::from(page));
    let page_size_mm = doc.page_size_mm_of(first);
    let bytes = match job.output {
        JobOutput::Pdf => JobBytes::Pdf(doc.pdf_selected(pages)?),
        JobOutput::Png { dpi } => {
            let (w, h) = page_size_mm;
            let px = |mm: f64| mm / 25.4 * f64::from(dpi);
            if px(w) * px(h) > limits.max_raster_pixels {
                return Err(DocsError::TooManyPixels);
            }
            JobBytes::Png(doc.png_selected(dpi, pages)?)
        }
    };
    Ok(JobResult {
        page_count: pages.map_or(doc.page_count(), |p| p.as_slice().len()),
        page_size_mm,
        bytes,
    })
}
