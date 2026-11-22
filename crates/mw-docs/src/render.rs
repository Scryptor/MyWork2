//! Compiling Typst source and turning the result into PDF and PNG.

use typst::diag::SourceDiagnostic;
use typst::utils::Scalar;
use typst_layout::PagedDocument;
use typst_pdf::PdfOptions;
use typst_render::RenderOptions;

use crate::world::DocsWorld;

#[derive(Debug, thiserror::Error)]
pub enum DocsError {
    /// Typst rejected the source; one line per error.
    #[error("document does not compile: {}", .0.join("; "))]
    Compile(Vec<String>),
    #[error("cannot produce PDF: {}", .0.join("; "))]
    Pdf(Vec<String>),
    #[error("cannot produce PNG: {0}")]
    Png(String),
    #[error("invalid virtual file name: {0}")]
    BadFileName(String),
    #[error("invalid resolution: {0} dpi")]
    Dpi(u32),
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

pub(crate) fn compile(
    source: String,
    files: Vec<(String, Vec<u8>)>,
) -> Result<Rendered, DocsError> {
    let world = DocsWorld::new(source, files)?;
    let output = typst::compile::<PagedDocument>(&world).output;
    evict_cache();
    match output {
        Ok(doc) => Ok(Rendered { doc }),
        Err(errors) => Err(DocsError::Compile(messages(errors))),
    }
}

impl Rendered {
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
