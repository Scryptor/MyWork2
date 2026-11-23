//! The author's own Typst code: one virtual source per code block and copy, and the errors Typst
//! finds in them as places in the author's code.
//!
//! A code file starts with [`CODE_HEADER_LINES`] lines of its own: the import of `/mw.typ` (the
//! helpers of the main file: included files do not see its bindings) and the copy it is in. The
//! main file includes it in place of the block.

use std::collections::HashMap;

use typst::World;
use typst::diag::{Severity, SourceDiagnostic};
use typst::syntax::{DiagSpan, DiagSpanKind, FileId};

use crate::model::{CodeDiagnostic, CodePlace};
use crate::world::file_id;

use super::page::CopyRole;

/// The lines a code file has before the author's code.
const CODE_HEADER_LINES: u32 = 2;
/// At most this many diagnostics: the first errors are the ones worth reading.
const MAX_DIAGNOSTICS: usize = 20;
/// The helpers every code file imports.
const HELPERS_PATH: &str = "/mw.typ";

/// The code of one block in one copy.
pub(super) struct CodeFile {
    path: String,
    /// The path of the block in the template: `[i]`, or `[i, column, j]` inside columns.
    block: Vec<usize>,
    text: String,
}

impl CodeFile {
    pub(super) fn new(block: &[usize], copy: CopyRole, code: &str) -> Self {
        let at: Vec<String> = block.iter().map(usize::to_string).collect();
        let role = copy.name();
        Self {
            path: format!("/code-{}-{role}.typ", at.join("-")),
            block: block.to_vec(),
            text: format!("#import \"{HELPERS_PATH}\": *\n#let copy = \"{role}\"\n{code}"),
        }
    }

    /// The path the main file includes.
    pub(super) fn path(&self) -> &str {
        &self.path
    }
}

/// The extra sources of a template with code: `/mw.typ` and the code files.
pub(crate) struct CodeSources {
    helpers: String,
    files: Vec<CodeFile>,
}

impl CodeSources {
    /// `helpers` is the text of `/mw.typ`.
    pub(super) fn new(helpers: String, files: Vec<CodeFile>) -> Self {
        Self { helpers, files }
    }

    /// The sources for the `World`, by path.
    pub(crate) fn sources(&self) -> Vec<(String, String)> {
        let helpers = (HELPERS_PATH.to_owned(), self.helpers.clone());
        let files = self.files.iter().map(|f| (f.path.clone(), f.text.clone()));
        std::iter::once(helpers).chain(files).collect()
    }

    /// The errors of a compilation in `world` as diagnostics of the author's code: one per place and
    /// message (both copies of a block report the same error), at most [`MAX_DIAGNOSTICS`]. Warnings
    /// are dropped.
    pub(crate) fn diagnostics(
        &self,
        world: &dyn World,
        errors: impl IntoIterator<Item = SourceDiagnostic>,
    ) -> Vec<CodeDiagnostic> {
        let blocks: HashMap<FileId, &[usize]> = self
            .files
            .iter()
            .filter_map(|f| Some((file_id(&f.path)?, f.block.as_slice())))
            .collect();
        let all = errors
            .into_iter()
            .filter(|error| error.severity == Severity::Error)
            .map(|error| CodeDiagnostic {
                place: place(world, &blocks, &error),
                message: error.message.to_string(),
                hints: error.hints.iter().map(|hint| hint.v.to_string()).collect(),
            });
        distinct(all)
    }
}

/// The first of the diagnostics with each place and message, at most [`MAX_DIAGNOSTICS`] of them.
fn distinct(diagnostics: impl IntoIterator<Item = CodeDiagnostic>) -> Vec<CodeDiagnostic> {
    let mut out: Vec<CodeDiagnostic> = Vec::new();
    for diagnostic in diagnostics {
        if !out
            .iter()
            .any(|d| d.place == diagnostic.place && d.message == diagnostic.message)
        {
            out.push(diagnostic);
        }
        if out.len() == MAX_DIAGNOSTICS {
            break;
        }
    }
    out
}

/// Where in the author's code `error` is: its own place if it is in a code file, otherwise the first
/// point of its trace in one (an error inside a helper points at the call in the code).
fn place(
    world: &dyn World,
    blocks: &HashMap<FileId, &[usize]>,
    error: &SourceDiagnostic,
) -> Option<CodePlace> {
    let trace = error
        .trace
        .iter()
        .map(|point| DiagSpan::from_span(point.span, None));
    std::iter::once(error.span)
        .chain(trace)
        .find_map(|span| code_place(world, blocks, span))
}

/// The place of `span` if it points into a code file. A place in the header is the start of the code.
fn code_place(
    world: &dyn World,
    blocks: &HashMap<FileId, &[usize]>,
    span: DiagSpan,
) -> Option<CodePlace> {
    let id = span.id()?;
    let block = blocks.get(&id)?;
    let source = world.source(id).ok()?;
    let start = match span.get() {
        DiagSpanKind::Number { num, sub_range, .. } => source.range(num, sub_range)?.start,
        DiagSpanKind::Range { range, .. } => range.start,
        DiagSpanKind::Detached => return None,
    };
    let (line, column) = source.lines().byte_to_line_column(start)?;
    // Typst counts both from 0; people count from 1, without the header.
    let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX).saturating_add(1);
    let (line, column) = match line.checked_sub(CODE_HEADER_LINES as usize) {
        Some(line) => (count(line), count(column)),
        None => (1, 1),
    };
    Some(CodePlace {
        block: block.to_vec(),
        line,
        column,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_header_has_its_lines() {
        let file = CodeFile::new(&[3, 1, 0], CopyRole::Client, "КОД");
        assert_eq!(file.path(), "/code-3-1-0-client.typ");
        let header = file
            .text
            .strip_suffix("КОД")
            .expect("the code ends the file");
        assert_eq!(header.lines().count(), CODE_HEADER_LINES as usize);
        assert!(header.ends_with('\n'));
    }

    fn diagnostic(line: u32, message: &str, hint: &str) -> CodeDiagnostic {
        CodeDiagnostic {
            place: Some(CodePlace {
                block: vec![0],
                line,
                column: 1,
            }),
            message: message.to_owned(),
            hints: vec![hint.to_owned()],
        }
    }

    #[test]
    fn a_place_and_message_are_given_once() {
        let got = distinct([
            diagnostic(1, "a", "first"),
            diagnostic(1, "a", "second"),
            diagnostic(2, "a", ""),
            diagnostic(1, "b", ""),
        ]);
        let got: Vec<(u32, &str, &str)> = got
            .iter()
            .map(|d| {
                let line = d.place.as_ref().map_or(0, |p| p.line);
                (line, d.message.as_str(), d.hints[0].as_str())
            })
            .collect();
        assert_eq!(got, [(1, "a", "first"), (2, "a", ""), (1, "b", "")]);
    }

    #[test]
    fn at_most_twenty_first_diagnostics() {
        let got = distinct((1..=30).map(|line| diagnostic(line, "a", "")));
        assert_eq!(got.len(), MAX_DIAGNOSTICS);
        let lines: Vec<u32> = got
            .iter()
            .filter_map(|d| d.place.as_ref().map(|p| p.line))
            .collect();
        assert_eq!(lines, (1..=20).collect::<Vec<_>>());
    }
}
