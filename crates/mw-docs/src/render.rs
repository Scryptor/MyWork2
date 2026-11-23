//! Compiling Typst source and turning the result into PDF and PNG.

use typst::diag::SourceDiagnostic;
use typst::utils::Scalar;
use typst_layout::PagedDocument;
use typst_pdf::PdfOptions;
use typst_render::RenderOptions;

use crate::fonts::FontSet;
use crate::model::CodeDiagnostic;
use crate::typst_gen::CodeSources;
use crate::world::DocsWorld;

#[derive(Debug, thiserror::Error)]
pub enum DocsError {
    /// Typst rejected the source; one line per error.
    #[error("document does not compile: {}", .0.join("; "))]
    Compile(Vec<String>),
    /// The author's own Typst code does not compile: every error of a template with code blocks.
    #[error("own Typst code does not compile: {}", code_messages(.0))]
    Code(Vec<CodeDiagnostic>),
    #[error("cannot produce PDF: {}", .0.join("; "))]
    Pdf(Vec<String>),
    #[error("cannot produce PNG: {0}")]
    Png(String),
    #[error("invalid virtual file name: {0}")]
    BadFileName(String),
    #[error("invalid resolution: {0} dpi")]
    Dpi(u32),
    /// The caller refused a document with more pages than it allows.
    #[error("document has {0} pages, more than allowed")]
    TooManyPages(usize),
    /// The caller refused a raster larger than it allows.
    #[error("raster page is too large")]
    TooManyPixels,
    /// The documents of a job took longer to compile than its time budget.
    #[error("rendering took longer than its budget")]
    TooSlow,
}

fn code_messages(diagnostics: &[CodeDiagnostic]) -> String {
    let messages: Vec<&str> = diagnostics.iter().map(|d| d.message.as_str()).collect();
    messages.join("; ")
}

/// A compiled document.
pub struct Rendered {
    doc: PagedDocument,
}

/// Typst memoizes layout results globally; without eviction a long-running server grows without bound.
pub(crate) fn evict_cache() {
    comemo::evict(30);
}

fn messages(errors: impl IntoIterator<Item = SourceDiagnostic>) -> Vec<String> {
    errors
        .into_iter()
        .map(|e| {
            let mut line = e.message.to_string();
            for hint in &e.hints {
                line.push_str(" (hint: ");
                line.push_str(&hint.v);
                line.push(')');
            }
            line
        })
        .collect()
}

/// Compiles `main` with the virtual `files` (name, bytes). `code` are the sources of a template with
/// the author's code: its errors, all of them, become [`DocsError::Code`].
pub(crate) fn compile(
    main: String,
    code: Option<CodeSources>,
    files: Vec<(String, Vec<u8>)>,
    fonts: &FontSet,
) -> Result<Rendered, DocsError> {
    let sources = code.as_ref().map(CodeSources::sources).unwrap_or_default();
    let world = DocsWorld::new(main, sources, files, fonts)?;
    let output = typst::compile::<PagedDocument>(&world).output;
    evict_cache();
    match (output, code) {
        (Ok(doc), _) => Ok(Rendered { doc }),
        (Err(errors), Some(code)) => Err(DocsError::Code(code.diagnostics(&world, errors))),
        (Err(errors), None) => Err(DocsError::Compile(messages(errors))),
    }
}

/// Склеивает документы в один: страницы по порядку, сведения о документе (`info`) берутся у первого.
/// Страницы уже разложены и сохраняют свои номера. Пустой список даёт пустой документ без страниц.
pub fn merge(docs: Vec<Rendered>) -> Rendered {
    let mut info = typst::model::DocumentInfo::default();
    let mut pages = typst::diag::EcoVec::new();
    for (i, mut rendered) in docs.into_iter().enumerate() {
        if i == 0 {
            info = std::mem::take(rendered.doc.info_mut());
        }
        pages.extend(rendered.doc.pages().iter().cloned());
    }
    Rendered {
        doc: PagedDocument::new(pages, info),
    }
}

impl Rendered {
    /// The laid-out document, for tests that look at the positions of what is printed.
    #[cfg(test)]
    pub(crate) fn document(&self) -> &PagedDocument {
        &self.doc
    }

    /// Families of the fonts the text of the document is set in. Only with the `testing` feature.
    #[cfg(feature = "testing")]
    pub fn font_families(&self) -> std::collections::BTreeSet<String> {
        fn walk(frame: &typst::layout::Frame, out: &mut std::collections::BTreeSet<String>) {
            use typst::layout::FrameItem;
            for (_, item) in frame.items() {
                match item {
                    FrameItem::Text(text) => {
                        out.insert(text.font.info().family.clone());
                    }
                    FrameItem::Group(group) => walk(&group.frame, out),
                    _ => {}
                }
            }
        }
        let mut out = std::collections::BTreeSet::new();
        for page in self.doc.pages() {
            walk(&page.frame, &mut out);
        }
        out
    }

    pub fn page_count(&self) -> usize {
        self.doc.pages().len()
    }

    /// Size of the first page in millimetres (width, height); `(0, 0)` for a document without pages.
    pub fn page_size_mm(&self) -> (f64, f64) {
        self.doc
            .pages()
            .first()
            .map(|p| {
                let size = p.frame.size();
                (size.x.to_mm(), size.y.to_mm())
            })
            .unwrap_or((0.0, 0.0))
    }

    pub fn pdf(&self) -> Result<Vec<u8>, DocsError> {
        let bytes = typst_pdf::pdf(&self.doc, &PdfOptions::default())
            .map_err(|e| DocsError::Pdf(messages(e)));
        evict_cache();
        bytes
    }

    /// One PNG per page at `dpi` dots per inch.
    pub fn png_pages(&self, dpi: u32) -> Result<Vec<Vec<u8>>, DocsError> {
        if dpi == 0 || dpi > 1200 {
            return Err(DocsError::Dpi(dpi));
        }
        let options = RenderOptions {
            pixel_per_pt: Scalar::new(f64::from(dpi) / 72.0),
            render_bleed: false,
        };
        let pages = self
            .doc
            .pages()
            .iter()
            .map(|page| {
                typst_render::render(page, &options)
                    .encode_png()
                    .map_err(|e| DocsError::Png(e.to_string()))
            })
            .collect();
        evict_cache();
        pages
    }

    /// All text of the document, page by page, a new line where the baseline changes.
    pub fn text(&self) -> String {
        let mut out = String::new();
        for page in self.doc.pages() {
            collect_text(&page.frame, &mut out);
            out.push('\n');
        }
        out
    }
}

/// Appends the text of a frame; group offsets are ignored (good enough for tests and checks).
fn collect_text(frame: &typst::layout::Frame, out: &mut String) {
    use typst::layout::FrameItem;
    let mut last_y = None;
    for (pos, item) in frame.items() {
        match item {
            FrameItem::Text(text) => {
                if last_y.is_some_and(|y| y != pos.y) {
                    out.push('\n');
                }
                last_y = Some(pos.y);
                out.push_str(&text.text);
            }
            FrameItem::Group(group) => collect_text(&group.frame, out),
            _ => {}
        }
    }
}
