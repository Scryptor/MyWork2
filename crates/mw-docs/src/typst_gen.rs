//! Template → Typst source.
//!
//! User data never becomes markup. [`to_typst`] sees only the template and the options; the values are read
//! inside the document from `data.json`, and template text goes in as escaped Typst string literals.

use std::collections::BTreeMap;
use std::fmt::Write;

use mw_domain::barcode::is_valid_upc_a;
use serde_json::{Map, Value};

use crate::barcode::{fit_module_mm, upc_a_size_mm, upc_a_svg};
use crate::fields::{FieldDef, field_by_placeholder, fields};
use crate::model::{Align, Block, BlockBody, CopyFilter, FieldRow, Layout, Template};
use crate::render::{DocsError, Rendered, compile};

/// Space between the two copies of [`Layout::TwoPerA4`]; the cut line runs in its middle.
const CUT_GAP_MM: f64 = 8.0;
/// Size of the barcode digits per millimetre of module width (0.33 mm → 7.9 pt), never below
/// [`DIGITS_MIN_PT`].
const DIGITS_PT_PER_MODULE_MM: f64 = 24.0;
const DIGITS_MIN_PT: f64 = 7.0;
/// Space between the bars and the digits under them.
const DIGITS_GAP_MM: f64 = 0.5;
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
    // `barcode` in `data.json` is set exactly when `barcode.svg` exists.
    let barcode = data
        .barcode
        .as_deref()
        .filter(|code| is_valid_upc_a(code))
        .zip(Generator::new(template, opts).svg)
        .and_then(|(code, (module, bars))| upc_a_svg(code, module, bars).map(|svg| (code, svg)));
    let mut files = vec![(
        "data.json".to_owned(),
        data_json(data, barcode.as_ref().map(|(code, _)| *code)),
    )];
    if let Some((_, svg)) = barcode {
        files.push(("barcode.svg".to_owned(), svg.into_bytes()));
    }
    compile(to_typst(template, opts), files)
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
/// `v(key)`: a non-empty value with its line breaks; empty — in a blank form a line to write on for
/// the order and the client fields (the firm's own fields stay empty), otherwise the field's empty
/// text. The line runs to the end of the text line; `line: short-line` — a short one, for centred
/// text. `v1(key)`: the same in one line, with a short line. `one-line(where, body)`: `body` without
/// wrapping, cut off at the right edge when too long (the outset keeps accents and descenders inside
/// the clip).
const PRELUDE: &str = r#"#set par(leading: 0.5em, spacing: 0.8em)
#set block(spacing: 0.8em)
#let vspace = v
#let data = json("data.json")
#let fill-line = box(width: 1fr, height: 0.7em, stroke: (bottom: 0.5pt))
#let short-line = box(width: 3.5cm, height: 0.7em, stroke: (bottom: 0.5pt))
#let raw-value(key) = data.values.at(key, default: "")
#let filled(key) = raw-value(key).trim() != ""
#let write-in(key) = data.blank and (key.starts-with("order.") or key.starts-with("client."))
#let v(key, line: fill-line) = {
  let s = raw-value(key)
  if s.trim() != "" { s.split("\n").join(linebreak()) } else if write-in(key) { line } else if not data.blank { empties.at(key, default: "") }
}
#let v1(key) = {
  let s = raw-value(key)
  if s.trim() != "" { s.split("\n").join(" ") } else if write-in(key) { short-line } else if not data.blank { empties.at(key, default: "") }
}
#let one-line(where, body) = layout(size => {
  let width = measure(body).width
  block(width: 100%, clip: true, outset: (top: 0.3em, bottom: 0.3em), if width <= size.width { align(where, body) } else { box(width: width + 1pt, body) })
})
"#;

/// Typst source of `template`. It never contains document data, so it is the same for every order.
pub fn to_typst(template: &Template, opts: &RenderOptions) -> String {
    let generator = Generator::new(template, opts);
    let page = &generator.page;
    let width = generator.area_mm.0;
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
            out.push_str(&generator.flow(&template.blocks, CopyRole::Single, width));
            out.push('\n');
        }
        Layout::TwoPerA4 => {
            let half = (generator.area_mm.1 - CUT_GAP_MM) / 2.0;
            let _ = writeln!(
                out,
                "#let copy-service = {}",
                generator.flow(&template.blocks, CopyRole::Service, width)
            );
            let _ = writeln!(
                out,
                "#let copy-client = {}",
                generator.flow(&template.blocks, CopyRole::Client, width)
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
#[derive(Clone, Copy)]
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
    page: Page,
    /// Width and height of the page area inside the margins.
    area_mm: (f64, f64),
    opts: RenderOptions,
    /// Module and bar height of `barcode.svg` (see [`Self::barcode_svg`]).
    svg: Option<(f64, f64)>,
}

impl Generator {
    fn new(template: &Template, opts: &RenderOptions) -> Self {
        let page = Page::new(template, opts);
        let mut generator = Self {
            page,
            area_mm: (
                page.width - page.left - page.right,
                page.height - page.top - page.bottom,
            ),
            opts: *opts,
            svg: None,
        };
        generator.svg = generator.barcode_svg(template);
        generator
    }

    /// Module and bar height of `barcode.svg`; `None` without Barcode blocks. Every Barcode block
    /// shows the lower part of this one image, scaled to its own module, so that the guard bars always
    /// stick out 5 of its modules. The SVG has the module of the first block and bars just long enough
    /// for every block: a block with module `m` showing bars `H` high needs `H / m` modules of them.
    fn barcode_svg(&self, template: &Template) -> Option<(f64, f64)> {
        let mut found = Vec::new();
        self.barcodes(template, &template.blocks, self.area_mm.0, &mut found);
        let (svg_module, _) = *found.first()?;
        let modules = found
            .iter()
            .map(|&(module, bars)| bars / module)
            .fold(0.0, f64::max);
        Some((svg_module, svg_module * modules))
    }

    /// Appends (module, the longest bars it shows) of every Barcode block in `blocks`, which are
    /// `width_mm` wide. A block that fills the page shows at most the page area.
    fn barcodes(
        &self,
        template: &Template,
        blocks: &[Block],
        width_mm: f64,
        found: &mut Vec<(f64, f64)>,
    ) {
        for block in blocks {
            match &block.body {
                BlockBody::Barcode {
                    height_mm, fill, ..
                } => {
                    let height = f64::from(*height_mm);
                    let bars = if *fill && template.layout == Layout::OnePerPage {
                        height.max(self.area_mm.1)
                    } else {
                        height
                    };
                    found.push((fit_module_mm(width_mm, self.opts.dpi), bars));
                }
                BlockBody::Columns { columns, gap_mm } => {
                    let width = column_width(width_mm, columns.len(), *gap_mm);
                    for column in columns {
                        self.barcodes(template, column, width, found);
                    }
                }
                _ => {}
            }
        }
    }

    /// A full-width block with `blocks` one under another: one Typst code expression. `width_mm` is
    /// the width of the column the blocks are in.
    fn flow(&self, blocks: &[Block], copy: CopyRole, width_mm: f64) -> String {
        let mut out = String::from("block(width: 100%, {\n");
        for block in blocks {
            let shown = match block.copies {
                CopyFilter::Both => true,
                CopyFilter::Service => copy == CopyRole::Service,
                CopyFilter::Client => copy == CopyRole::Client,
            };
            if shown {
                self.block(&mut out, &block.body, copy, width_mm);
            }
        }
        out.push_str("})");
        out
    }

    /// Appends one code expression and a line break (Typst joins the lines of a code block).
    fn block(&self, out: &mut String, body: &BlockBody, copy: CopyRole, width_mm: f64) {
        match body {
            BlockBody::Text {
                text,
                size_pt,
                bold,
                align,
                one_line,
            } => {
                let mut style = String::new();
                if let Some(size) = size_pt {
                    let _ = write!(style, "size: {}, ", pt(f64::from(*size)));
                }
                if *bold {
                    style.push_str("weight: \"bold\", ");
                }
                if *one_line {
                    // The text style is set around the block, so that the em of its spacing and of the
                    // clip outset is this text's. One-line blocks stack like the lines of a paragraph.
                    let _ = write!(
                        out,
                        "{{ set text({style}); set block(spacing: 0.5em); one-line({}, {}) }}",
                        align_name(*align),
                        one_line_expr(&pieces(text, true))
                    );
                } else {
                    // A line to write on runs to the end of left-aligned text; elsewhere it would push
                    // the text to the edges.
                    let value = if *align == Align::Left {
                        expr(&pieces(text, true))
                    } else {
                        short_line_expr(&pieces(text, true))
                    };
                    let _ = write!(
                        out,
                        "block(width: 100%, align({}, text({style}{value})))",
                        align_name(*align),
                    );
                }
            }
            BlockBody::Fields { rows } => {
                out.push_str("{\nlet cells = ()\n");
                for row in rows {
                    field_row(out, row);
                }
                // A blank form gets room to write and only its lines to write on.
                out.push_str(
                    "if cells.len() > 0 { table(columns: (auto, 1fr), \
                     stroke: if data.blank { none } else { (x: none, y: 0.3pt + luma(170)) }, \
                     inset: (x: 1mm, y: if data.blank { 2mm } else { 1.2mm }), ..cells) }\n}",
                );
            }
            BlockBody::Barcode {
                height_mm,
                digits,
                align,
                fill,
            } => {
                let module = fit_module_mm(width_mm, self.opts.dpi);
                let (w, h) = upc_a_size_mm(module, f64::from(*height_mm));
                // The whole image at this block's module: its natural size, scaled.
                let image_h = self.svg.map_or(h, |(svg_module, bars)| {
                    upc_a_size_mm(svg_module, bars).1 * module / svg_module
                });
                let under_bars = if *digits { ", digits" } else { "" };
                let digits = if *digits {
                    format!(
                        "align(center, text(size: {}, data.barcode))",
                        pt((module * DIGITS_PT_PER_MODULE_MM).max(DIGITS_MIN_PT))
                    )
                } else {
                    "none".to_owned()
                };
                // The lower `height` of the image (see `barcode_svg`), at this block's module.
                let symbol = |height: &str| {
                    format!(
                        "align({}, block(width: {w}, stack(spacing: {gap}, box(width: {w}, \
                         height: {height}, clip: true, place(bottom + left, image(\"barcode.svg\", \
                         width: {w}, height: {image_h}, fit: \"stretch\"))){under_bars})))",
                        align_name(*align),
                        w = mm(w),
                        image_h = mm(image_h),
                        gap = mm(DIGITS_GAP_MM),
                    )
                };
                if *fill && copy == CopyRole::Single {
                    // The block takes the height the other blocks leave; the bars get it less the digits.
                    let _ = write!(
                        out,
                        "if data.barcode != none {{ block(width: 100%, height: 1fr, layout(size => {{
  let digits = {digits}
  let free = size.height - if digits == none {{ 0pt }} else {{ measure(digits).height + {gap} }}
  let height = calc.max(free, {h})
  {symbol}
}})) }}",
                        gap = mm(DIGITS_GAP_MM),
                        h = mm(h),
                        symbol = symbol("height"),
                    );
                } else {
                    let _ = write!(
                        out,
                        "if data.barcode != none {{ let digits = {digits}; {} }}",
                        symbol(&mm(h))
                    );
                }
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
                    let width = column_width(width_mm, columns.len(), *gap_mm);
                    for column in columns {
                        out.push_str(",\n");
                        out.push_str(&self.flow(column, copy, width));
                    }
                    out.push(')');
                }
            }
            BlockBody::CopyLabel { align } => {
                let label = match copy {
                    CopyRole::Single => None,
                    CopyRole::Service => Some(SERVICE_COPY),
                    CopyRole::Client => Some(CLIENT_COPY),
                };
                if let Some(label) = label {
                    let _ = write!(
                        out,
                        "block(width: 100%, align({}, text(style: \"italic\", {})))",
                        align_name(*align),
                        typst_str(label)
                    );
                }
            }
            BlockBody::CalibrationMarks => self.calibration_marks(out),
        }
        out.push('\n');
    }

    /// See [`BlockBody::CalibrationMarks`]. The marks are as thick as two dots of a 203 dpi printer.
    fn calibration_marks(&self, out: &mut String) {
        const EDGE_MM: f64 = 0.3;
        const STROKE_MM: f64 = 0.25;
        let (width, height) = self.area_mm;
        // The outer edge of the frame line is `EDGE_MM` inside the area.
        let inset = EDGE_MM + STROKE_MM / 2.0;
        let mut size = format!(
            "{} × {} мм",
            decimal(self.page.width),
            decimal(self.page.height)
        );
        if let Some(dpi) = self.opts.dpi {
            let _ = write!(size, " · {dpi} dpi");
        }
        let (dx, dy) = self.opts.offset_mm;
        let offset = format!(
            "смещение {}/{}",
            decimal(f64::from(dx)),
            decimal(f64::from(dy))
        );
        let _ = write!(
            out,
            "block(width: 100%, height: {height}, {{
place(top + left, dx: {inset}, dy: {inset}, rect(width: 100% - {inset2}, height: 100% - {inset2}, stroke: {stroke}))
place(center + horizon, line(length: {cross}, stroke: {stroke}))
place(center + horizon, line(length: {cross}, angle: 90deg, stroke: {stroke}))
place(center + bottom, dy: -1.5mm, align(center, text(size: 7pt, ({size}, linebreak(), {offset}).join())))
}})",
            height = mm(height),
            inset = mm(inset),
            inset2 = mm(2.0 * inset),
            stroke = mm(STROKE_MM),
            cross = mm(width.min(height) * 0.3),
            size = typst_str(&size),
            offset = typst_str(&offset),
        );
    }
}

/// Width of each of `count` equal columns `gap_mm` apart in `width_mm`.
fn column_width(width_mm: f64, count: usize, gap_mm: f32) -> f64 {
    let count = count.max(1) as f64;
    (width_mm - f64::from(gap_mm) * (count - 1.0)) / count
}

/// A number for people: at most two decimals, a decimal comma and a real minus (`-1.5` → «−1,5»).
fn decimal(x: f64) -> String {
    let x = if x.is_finite() { x } else { 0.0 };
    let s = format!("{:.2}", x.abs());
    let s = s
        .trim_end_matches('0')
        .trim_end_matches('.')
        .replace('.', ",");
    if x < 0.0 && s != "0" {
        format!("−{s}")
    } else {
        s
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

/// [`expr`] with short lines to write on (centred text).
fn short_line_expr(pieces: &[Piece]) -> String {
    let parts: Vec<String> = pieces
        .iter()
        .map(|p| match p {
            Piece::Field(field) => format!("v({}, line: short-line)", typst_str(field.key)),
            other => expr(std::slice::from_ref(other)),
        })
        .collect();
    join_parts(parts)
}

/// One Typst code expression for `pieces` in one line: values through `v1`, line breaks as spaces.
fn one_line_expr(pieces: &[Piece]) -> String {
    let parts: Vec<String> = pieces
        .iter()
        .map(|p| match p {
            Piece::Text(text) => typst_str(text),
            Piece::Field(field) => format!("v1({})", typst_str(field.key)),
            Piece::Break => "\" \"".to_owned(),
        })
        .collect();
    join_parts(parts)
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
    join_parts(parts)
}

fn join_parts(parts: Vec<String>) -> String {
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
