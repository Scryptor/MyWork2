//! Documents and printing: Typst runs inside the process, with embedded fonts. Besides them it reads fallback fonts from the folders and the system
//! it is told about ([`configure_fonts`]), and does no other I/O, except in the mode of a separate
//! render process ([`worker_main`]): there it reads jobs from stdin and writes the results to stdout.
//! A [`RenderPool`] keeps such processes on the parent's side and kills one whose job runs too long.
//!
//! A [`model::Template`] becomes Typst source ([`to_typst`]); [`render`] compiles it with the document
//! data, which the source reads from a JSON file and never contains. The result is a [`Rendered`]
//! document that turns into PDF or PNG pages. A [`Job`] ([`run_job`]) does all of it for one or many
//! documents of one template, within limits of pages, pixels and time.

mod barcode;
pub mod fields;
mod fonts;
mod job;
#[cfg(test)]
mod layout_tests;
pub mod model;
mod pool;
mod qr;
mod render;
mod standard;
mod typst_gen;
mod wire;
mod worker;
mod world;

pub use barcode::{Symbology, code128_svg, module_mm, symbology, upc_a_svg};
pub use fonts::{FontSet, FontSources, configure_fonts, font_covers, warm_fonts};
pub use job::{Job, JobBytes, JobLimits, JobOutput, JobResult, run_job, run_job_with_fonts};
pub use pool::{RenderPool, WorkerLimits};
pub use render::{DocsError, Rendered, merge};
pub use standard::{sample_data, standard_template};
pub use typst_gen::{DocumentData, RenderOptions, block_code, render, render_with_fonts, to_typst};
pub use worker::{WORKER_ARG, serve, worker_main};

/// Test hook: `error` sent by the render process in its `Failed` reply and read back the way the
/// parent reads it. Only with the `testing` feature.
#[cfg(feature = "testing")]
pub fn error_through_wire_for_tests(error: DocsError) -> std::io::Result<DocsError> {
    let mut pipe = Vec::new();
    let failed = wire::Reply::Failed {
        error: error.into(),
    };
    wire::send(&mut pipe, &failed, &[])?;
    match wire::receive(&mut pipe.as_slice())? {
        Some(wire::Reply::Failed { error }) => Ok(error.into()),
        _ => Err(wire::protocol("ждали Failed")),
    }
}

/// Test hook: compiles Typst `source` with virtual `files` (name, bytes). Only with the `testing` feature.
#[cfg(feature = "testing")]
pub fn compile_for_tests(
    source: String,
    files: Vec<(String, Vec<u8>)>,
) -> Result<Rendered, DocsError> {
    render::compile(source, None, files, fonts::shared())
}

/// Test hook: like [`compile_for_tests`] on the given set of fonts. Only with the `testing` feature.
#[cfg(feature = "testing")]
pub fn compile_with_fonts_for_tests(
    source: String,
    files: Vec<(String, Vec<u8>)>,
    fonts: &FontSet,
) -> Result<Rendered, DocsError> {
    render::compile(source, None, files, fonts)
}
