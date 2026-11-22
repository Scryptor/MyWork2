//! Documents and printing: Typst runs inside the process, with embedded fonts and no I/O of its own.
//!
//! A [`model::Template`] becomes Typst source ([`to_typst`]); [`render`] compiles it with the document
//! data, which the source reads from a JSON file and never contains. The result is a [`Rendered`]
//! document that turns into PDF or PNG pages.

mod barcode;
pub mod fields;
mod fonts;
pub mod model;
mod render;
mod standard;
mod typst_gen;
mod world;

pub use barcode::{module_mm, upc_a_svg};
pub use fonts::font_covers;
pub use render::{DocsError, Rendered};
pub use standard::{sample_data, standard_template};
pub use typst_gen::{DocumentData, RenderOptions, render, to_typst};

/// Test hook: compiles Typst `source` with virtual `files` (name, bytes). Only with the `testing` feature.
#[cfg(feature = "testing")]
pub fn compile_for_tests(
    source: String,
    files: Vec<(String, Vec<u8>)>,
) -> Result<Rendered, DocsError> {
    render::compile(source, files)
}
