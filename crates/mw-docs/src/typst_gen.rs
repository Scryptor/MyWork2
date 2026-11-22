//! Template → Typst source.
//!
//! User data never becomes markup. [`to_typst`] sees only the template and the options; the values are read
//! inside the document from `data.json`, and template text goes in as escaped Typst string literals.

use std::collections::BTreeMap;
use std::fmt::Write;

use mw_domain::barcode::is_valid_upc_a;
use serde_json::{Map, Value};

use crate::barcode::{module_mm, upc_a_size_mm, upc_a_svg};
use crate::fields::{FieldDef, field_by_placeholder, fields};
use crate::model::{Align, Block, BlockBody, CopyFilter, FieldRow, Layout, Template};
use crate::render::{DocsError, Rendered, compile};

/// Space between the two copies of [`Layout::TwoPerA4`]; the cut line runs in its middle.
const CUT_GAP_MM: f64 = 8.0;
/// Size of the barcode digits per millimetre of module width (0.33 mm → 7.9 pt).
const DIGITS_PT_PER_MODULE_MM: f64 = 24.0;
const SERVICE_COPY: &str = "Экземпляр сервиса";
const CLIENT_COPY: &str = "Экземпляр клиента";

/// The values of one document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocumentData {
    /// By field key (`order.number`, see [`crate::fields`]); a missing key is an empty value.
    pub values: BTreeMap<String, String>,
    /// UPC-A digits; `None` or an invalid code prints no barcode.
    pub barcode: Option<String>,
    /// A form to fill in by hand: empty values become lines to write on.
    pub blank: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RenderOptions {
    /// Label (width, height) in mm; replaces the template's page size (stickers, calibration).
    pub label_mm: Option<(f32, f32)>,
    /// Shift of the page content in mm: x to the right, y down.
    pub offset_mm: (f32, f32),
    /// Printer resolution: the barcode module becomes a whole number of printer dots. `None` — 0.33 mm.
    pub dpi: Option<u32>,
}

/// Compiles `template` with `data`.
pub fn render(
    template: &Template,
    data: &DocumentData,
    opts: &RenderOptions,
) -> Result<Rendered, DocsError> {
    // `barcode` in `data.json` is set exactly when `barcode.svg` exists. One SVG serves every Barcode
    // block: each block stretches it to its own height, the module width stays.
    let barcode = data
        .barcode
        .as_deref()
        .filter(|code| is_valid_upc_a(code))
        .zip(first_barcode_height(&template.blocks))
        .and_then(|(code, height)| {
            upc_a_svg(code, module_mm(opts.dpi), f64::from(height)).map(|svg| (code, svg))
        });
    let mut files = vec![(
        "data.json".to_owned(),
        data_json(data, barcode.as_ref().map(|(code, _)| *code)),
    )];
    if let Some((_, svg)) = barcode {
        files.push(("barcode.svg".to_owned(), svg.into_bytes()));
    }
    compile(to_typst(template, opts), files)
}

fn first_barcode_height(blocks: &[Block]) -> Option<f32> {
    blocks.iter().find_map(|block| match &block.body {
        BlockBody::Barcode { height_mm, .. } => Some(*height_mm),
        BlockBody::Columns { columns, .. } => columns.iter().find_map(|c| first_barcode_height(c)),
        _ => None,
    })
}

fn data_json(data: &DocumentData, barcode: Option<&str>) -> Vec<u8> {
    let values: Map<String, Value> = data
        .values
        .iter()
        .map(|(key, value)| {
            let value = value.replace("\r\n", "\n").replace('\r', "\n");
            (key.clone(), Value::String(value))
        })
        .collect();
    let mut root = Map::new();
    root.insert("values".to_owned(), Value::Object(values));
    root.insert(
        "barcode".to_owned(),
        barcode.map_or(Value::Null, |code| Value::String(code.to_owned())),
    );
    root.insert("blank".to_owned(), Value::Bool(data.blank));
    Value::Object(root).to_string().into_bytes()
}

/// Helpers every document starts with (after `empties`, which `v` captures).
/// `v(key)`: a non-empty value with its line breaks; empty — a line in a blank form, otherwise the
/// field's empty text.
const PRELUDE: &str = r#"#set par(leading: 0.5em, spacing: 0.8em)
#set block(spacing: 0.8em)
#let vspace = v
#let data = json("data.json")
#let fill-line = box(width: 1fr, height: 0.7em, stroke: (bottom: 0.5pt))
#let raw-value(key) = data.values.at(key, default: "")
#let filled(key) = raw-value(key).trim() != ""
#let v(key) = {
  let s = raw-value(key)
  if s.trim() != "" { s.split("\n").join(linebreak()) } else if data.blank { fill-line } else { empties.at(key, default: "") }
}
"#;

/// Typst source of `template`. It never contains document data, so it is the same for every order.
pub fn to_typst(template: &Template, opts: &RenderOptions) -> String {
    let page = Page::new(template, opts);
    let generator = Generator {
        module_mm: module_mm(opts.dpi),
    };
    let mut out = String::new();
    let _ = writeln!(
        out,
        "#set page(width: {}, height: {}, margin: (top: {}, right: {}, bottom: {}, left: {}))",
        mm(page.width),
        mm(page.height),
        mm(page.top),
        mm(page.right),
        mm(page.bottom),
        mm(page.left)
    );
    let _ = writeln!(
        out,
        "#set text(font: \"Libertinus Serif\", size: {}, lang: \"ru\")",
        pt(f64::from(template.font_size_pt))
    );
    let empties: Vec<String> = fields()
        .iter()
        .filter(|f| !f.empty.is_empty())
        .map(|f| format!("{}: {}", typst_str(f.key), typst_str(f.empty)))
        .collect();
    if empties.is_empty() {
        out.push_str("#let empties = (:)\n");
    } else {
        let _ = writeln!(out, "#let empties = ({},)", empties.join(", "));
    }
    out.push_str(PRELUDE);
    match template.layout {
        Layout::OnePerPage => {
            out.push('#');
            out.push_str(&generator.flow(&template.blocks, CopyRole::Single));
            out.push('\n');
        }
        Layout::TwoPerA4 => {
            let half = (page.height - page.top - page.bottom - CUT_GAP_MM) / 2.0;
            let width = page.width - page.left - page.right;
            let _ = writeln!(
                out,
                "#let copy-service = {}",
                generator.flow(&template.blocks, CopyRole::Service)
            );
            let _ = writeln!(
                out,
                "#let copy-client = {}",
                generator.flow(&template.blocks, CopyRole::Client)
            );
            let _ = writeln!(
                out,
                "#context {{
  let fits(c) = measure(c, width: {width}).height <= {half}
  if fits(copy-service) and fits(copy-client) {{
    place(top + left, copy-service)
    place(top + left, dx: {dx}, dy: {cut}, line(length: {page_width}, stroke: (thickness: 0.4pt, dash: \"dashed\")))
    place(top + left, dy: {client}, copy-client)
  }} else {{
    copy-service
    pagebreak()
    copy-client
  }}
}}",
                width = mm(width),
                half = mm(half),
                dx = mm(-page.left),
                cut = mm(half + CUT_GAP_MM / 2.0),
                page_width = mm(page.width),
                client = mm(half + CUT_GAP_MM),
            );
        }
    }
    out
}

/// Page size and margins in mm, the content offset already applied to the margins.
struct Page {
    width: f64,
    height: f64,
    top: f64,
    right: f64,
    bottom: f64,
    left: f64,
}

impl Page {
    fn new(template: &Template, opts: &RenderOptions) -> Self {
        let (width, height) = match opts.label_mm {
            Some(size) => size,
            None => {
                let (w, h) = template.page.size.size_mm();
                if template.page.landscape {
                    (h, w)
                } else {
                    (w, h)
                }
            }
        };
        let m = template.page.margins_mm;
        let (dx, dy) = (f64::from(opts.offset_mm.0), f64::from(opts.offset_mm.1));
        Self {
            width: f64::from(width),
            height: f64::from(height),
            top: f64::from(m.top) + dy,
            right: f64::from(m.right) - dx,
            bottom: f64::from(m.bottom) - dy,
            left: f64::from(m.left) + dx,
        }
    }
}

/// Which copy is being generated.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CopyRole {
    /// [`Layout::OnePerPage`]: the only copy, without copy-specific blocks.
    Single,
    Service,
    Client,
}

struct Generator {
    module_mm: f64,
}

impl Generator {
    /// A full-width block with `blocks` one under another: one Typst code expression.
    fn flow(&self, blocks: &[Block], copy: CopyRole) -> String {
        let mut out = String::from("block(width: 100%, {\n");
        for block in blocks {
            let shown = match block.copies {
                CopyFilter::Both => true,
                CopyFilter::Service => copy == CopyRole::Service,
                CopyFilter::Client => copy == CopyRole::Client,
            };
            if shown {
                self.block(&mut out, &block.body, copy);
            }
        }
        out.push_str("})");
        out
    }

    /// Appends one code expression and a line break (Typst joins the lines of a code block).
    fn block(&self, out: &mut String, body: &BlockBody, copy: CopyRole) {
        match body {
            BlockBody::Text {
                text,
                size_pt,
                bold,
                align,
            } => {
                let mut style = String::new();
                if let Some(size) = size_pt {
                    let _ = write!(style, "size: {}, ", pt(f64::from(*size)));
                }
                if *bold {
                    style.push_str("weight: \"bold\", ");
                }
                let _ = write!(
                    out,
                    "block(width: 100%, align({}, text({style}{})))",
                    align_name(*align),
                    expr(&pieces(text, true))
                );
            }
            BlockBody::Fields { rows } => {
                out.push_str("{\nlet cells = ()\n");
                for row in rows {
                    field_row(out, row);
                }
                out.push_str(
                    "if cells.len() > 0 { table(columns: (auto, 1fr), stroke: (x: none, y: 0.3pt + luma(170)), \
                     inset: (x: 1mm, y: 0.8mm), ..cells) }\n}",
                );
            }
            BlockBody::Barcode {
                height_mm,
                digits,
                align,
            } => {
                let (w, h) = upc_a_size_mm(self.module_mm, f64::from(*height_mm));
                let digits = if *digits {
                    format!(
                        ", align(center, text(size: {}, data.barcode))",
                        pt(self.module_mm * DIGITS_PT_PER_MODULE_MM)
                    )
                } else {
                    String::new()
                };
                let _ = write!(
                    out,
                    "if data.barcode != none {{ align({}, block(width: {w}, stack(spacing: 0.5mm, \
                     image(\"barcode.svg\", width: {w}, height: {h}, fit: \"stretch\"){digits}))) }}",
                    align_name(*align),
                    w = mm(w),
                    h = mm(h),
                );
            }
            BlockBody::Signatures { left, right } => {
                let _ = write!(
                    out,
                    "grid(columns: (1fr, 1fr), column-gutter: 10mm, ({}, \" \", fill-line).join(), \
                     ({}, \" \", fill-line).join())",
                    expr(&pieces(left, true)),
                    expr(&pieces(right, true))
                );
            }
            BlockBody::Separator => out.push_str("line(length: 100%, stroke: 0.5pt)"),
            BlockBody::Spacer { mm: space } => {
                let _ = write!(out, "vspace({})", mm(f64::from(*space)));
            }
            BlockBody::Columns { columns, gap_mm } => {
                if !columns.is_empty() {
                    let _ = write!(
                        out,
                        "grid(columns: ({}), column-gutter: {}",
                        "1fr, ".repeat(columns.len()),
                        mm(f64::from(*gap_mm))
                    );
                    for column in columns {
                        out.push_str(",\n");
                        out.push_str(&self.flow(column, copy));
                    }
                    out.push(')');
                }
            }
            BlockBody::CopyLabel => {
                let label = match copy {
                    CopyRole::Single => None,
                    CopyRole::Service => Some(SERVICE_COPY),
                    CopyRole::Client => Some(CLIENT_COPY),
                };
                if let Some(label) = label {
                    let _ = write!(out, "block(text(style: \"italic\", {}))", typst_str(label));
                }
            }
        }
        out.push('\n');
    }
}

/// Appends a row to the `cells` of a fields table, under the row's `hide_if_empty` condition.
fn field_row(out: &mut String, row: &FieldRow) {
    let value = pieces(&row.value, true);
    let cells = format!(
        "cells += ({}, {})",
        expr(&pieces(&row.label, false)),
        expr(&value)
    );
    let keys: Vec<String> = value
        .iter()
        .filter_map(|p| match p {
            Piece::Field(f) => Some(format!("filled({})", typst_str(f.key))),
            _ => None,
        })
        .collect();
    let has_text = value
        .iter()
        .any(|p| matches!(p, Piece::Text(t) if !t.trim().is_empty()));
    if !row.hide_if_empty {
        let _ = writeln!(out, "{cells}");
    } else if !keys.is_empty() {
        let _ = writeln!(out, "if data.blank or {} {{ {cells} }}", keys.join(" or "));
    } else if has_text {
        let _ = writeln!(out, "{cells}");
    }
}

fn align_name(align: Align) -> &'static str {
    match align {
        Align::Left => "left",
        Align::Center => "center",
        Align::Right => "right",
    }
}

enum Piece {
    Text(String),
    Field(&'static FieldDef),
    Break,
}

/// Splits template text into literal text, line breaks and (with `substitutions`) known `{placeholders}`.
fn pieces(text: &str, substitutions: bool) -> Vec<Piece> {
    let mut pieces = Vec::new();
    let mut literal = String::new();
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        let mut len = c.len_utf8();
        match c {
            '{' if substitutions => {
                // A name runs to the next brace; a `{` there means this one is literal text.
                let field = rest[1..]
                    .find(['{', '}'])
                    .filter(|&end| rest[1 + end..].starts_with('}'))
                    .and_then(|end| field_by_placeholder(&rest[1..=end]).map(|f| (f, end + 2)));
                match field {
                    Some((field, consumed)) => {
                        flush(&mut pieces, &mut literal);
                        pieces.push(Piece::Field(field));
                        len = consumed;
                    }
                    None => literal.push(c),
                }
            }
            '\r' | '\n' => {
                flush(&mut pieces, &mut literal);
                pieces.push(Piece::Break);
                if rest.starts_with("\r\n") {
                    len = 2;
                }
            }
            _ => literal.push(c),
        }
        rest = &rest[len..];
    }
    flush(&mut pieces, &mut literal);
    pieces
}

fn flush(pieces: &mut Vec<Piece>, literal: &mut String) {
    if !literal.is_empty() {
        pieces.push(Piece::Text(std::mem::take(literal)));
    }
}

/// One Typst code expression for `pieces`.
fn expr(pieces: &[Piece]) -> String {
    let parts: Vec<String> = pieces
        .iter()
        .map(|p| match p {
            Piece::Text(text) => typst_str(text),
            Piece::Field(field) => format!("v({})", typst_str(field.key)),
            Piece::Break => "linebreak()".to_owned(),
        })
        .collect();
    match parts.as_slice() {
        [] => "\"\"".to_owned(),
        [one] => one.clone(),
        _ => format!("({}).join()", parts.join(", ")),
    }
}

/// A Typst string literal: the text exactly, whatever characters it has.
fn typst_str(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{{{:x}}}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A number for Typst; a non-finite one (a broken template) becomes 0, so the source stays valid.
fn number(x: f64) -> String {
    format!("{:.3}", if x.is_finite() { x } else { 0.0 })
}

fn mm(x: f64) -> String {
    format!("{}mm", number(x))
}

fn pt(x: f64) -> String {
    format!("{}pt", number(x))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_literals_escape_quotes_backslashes_and_controls() {
        assert_eq!(typst_str(r#"a\b"c"#), r#""a\\b\"c""#);
        assert_eq!(typst_str("x\ny\t\u{7}"), r#""x\ny\t\u{7}""#);
        assert_eq!(
            typst_str("#set page() $x$ // к"),
            "\"#set page() $x$ // к\""
        );
    }

    #[test]
    fn pieces_find_known_placeholders_only() {
        let p = pieces("№ {заказ.номер}{нет}\r\n{{клиент.фио}", true);
        assert_eq!(
            expr(&p),
            r#"("№ ", v("order.number"), "{нет}", linebreak(), "{", v("client.name")).join()"#
        );
        assert_eq!(expr(&pieces("{заказ.номер}", false)), r#""{заказ.номер}""#);
        assert_eq!(expr(&pieces("", true)), r#""""#);
    }
}
