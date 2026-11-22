//! Documents and printing: Typst runs inside the process, with embedded fonts and no I/O of its own.
//!
//! Later layers build Typst source from a template and compile it; the result is a [`Rendered`]
//! document that turns into PDF or PNG pages.

mod barcode;
mod fonts;
mod render;
mod world;

pub use barcode::{module_mm, upc_a_svg};
pub use fonts::font_covers;
pub use render::{DocsError, Rendered};

/// Test hook: compiles Typst `source` with virtual `files` (name, bytes). Only with the `testing` feature.
#[cfg(feature = "testing")]
pub fn compile_for_tests(
    source: String,
    files: Vec<(String, Vec<u8>)>,
) -> Result<Rendered, DocsError> {
    render::compile(source, files)
}
