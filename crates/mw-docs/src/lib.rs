//! Documents and printing: Typst runs inside the process, with embedded fonts. Besides them it reads fallback fonts from the folders and the system
//! it is told about ([`configure_fonts`]), and does no other I/O.
//!
//! A [`model::Template`] becomes Typst source ([`to_typst`]); [`render`] compiles it with the document
//! data, which the source reads from a JSON file and never contains. The result is a [`Rendered`]
//! document that turns into PDF or PNG pages.

mod barcode;
pub mod fields;
mod fonts;
#[cfg(test)]
mod layout_tests;
pub mod model;
mod render;
mod standard;
mod typst_gen;
mod world;

pub use barcode::{Symbology, code128_svg, module_mm, symbology, upc_a_svg};
pub use fonts::{FontSet, FontSources, configure_fonts, font_covers, warm_fonts};
pub use render::{DocsError, Rendered, merge};
pub use standard::{sample_data, standard_template};
pub use typst_gen::{DocumentData, RenderOptions, render, render_with_fonts, to_typst};

/// Test hook: compiles Typst `source` with virtual `files` (name, bytes). Only with the `testing` feature.
#[cfg(feature = "testing")]
pub fn compile_for_tests(
    source: String,
    files: Vec<(String, Vec<u8>)>,
) -> Result<Rendered, DocsError> {
    render::compile(source, files, fonts::shared())
}

/// Test hook: like [`compile_for_tests`] on the given set of fonts. Only with the `testing` feature.
#[cfg(feature = "testing")]
pub fn compile_with_fonts_for_tests(
    source: String,
    files: Vec<(String, Vec<u8>)>,
    fonts: &FontSet,
) -> Result<Rendered, DocsError> {
    render::compile(source, files, fonts)
}
