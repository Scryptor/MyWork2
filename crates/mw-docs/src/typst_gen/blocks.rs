//! The Typst code of every [`BlockBody`].

use std::fmt::Write;

use crate::barcode::{fit_module_mm, stretch_module_mm, upc_a_size_mm};
use crate::model::{Align, Block, BlockBody, CopyFilter, FieldRow, Layout, Template};

use super::RenderOptions;
use super::page::{CopyRole, Page, decimal, mm, pt};
use super::text::{Piece, TextMode, literal_expr, pieces, text_expr, typst_str};

/// Size of the barcode digits per millimetre of module width (0.33 mm → 7.9 pt), never below
/// [`DIGITS_MIN_PT`].
const DIGITS_PT_PER_MODULE_MM: f64 = 24.0;
const DIGITS_MIN_PT: f64 = 7.0;
/// Space between the bars and the digits under them.
const DIGITS_GAP_MM: f64 = 0.5;
const SERVICE_COPY: &str = "Экземпляр сервиса";
const CLIENT_COPY: &str = "Экземпляр клиента";

pub(super) struct Generator {
    pub(super) page: Page,
    /// Width and height of the page area inside the margins.
    pub(super) area_mm: (f64, f64),
    opts: RenderOptions,
    /// Module and bar height of `barcode.svg` (see [`Self::barcode_svg`]).
    pub(super) svg: Option<(f64, f64)>,
}

impl Generator {
    pub(super) fn new(template: &Template, opts: &RenderOptions) -> Self {
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
                    height_mm,
                    fill,
                    stretch,
                    ..
                } => {
                    let height = f64::from(*height_mm);
                    let bars = if *fill && template.layout == Layout::OnePerPage {
                        height.max(self.area_mm.1)
                    } else {
                        height
                    };
                    found.push((self.module_mm(width_mm, *stretch), bars));
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

    /// The barcode module in a column `width_mm` wide.
    fn module_mm(&self, width_mm: f64, stretch: bool) -> f64 {
        if stretch {
            stretch_module_mm(width_mm, self.opts.dpi)
        } else {
            fit_module_mm(width_mm, self.opts.dpi)
        }
    }

    /// A full-width block with `blocks` one under another: one Typst code expression. `width_mm` is
    /// the width of the column the blocks are in.
    pub(super) fn flow(&self, blocks: &[Block], copy: CopyRole, width_mm: f64) -> String {
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
                        text_expr(text, TextMode::OneLine)
                    );
                } else {
                    // A line to write on runs to the end of left-aligned text; elsewhere it would push
                    // the text to the edges.
                    let mode = if *align == Align::Left {
                        TextMode::Wrap
                    } else {
                        TextMode::ShortLine
                    };
                    let value = text_expr(text, mode);
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
                stretch,
            } => {
                let module = self.module_mm(width_mm, *stretch);
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
                    text_expr(left, TextMode::Wrap),
                    text_expr(right, TextMode::Wrap)
                );
            }
            BlockBody::Separator {} => out.push_str("line(length: 100%, stroke: 0.5pt)"),
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
            BlockBody::CalibrationMarks {} => self.calibration_marks(out),
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

/// Appends a row to the `cells` of a fields table, under the row's `hide_if_empty` condition.
fn field_row(out: &mut String, row: &FieldRow) {
    let value = pieces(&row.value, true);
    let (label, value_expr) = (
        literal_expr(&row.label),
        text_expr(&row.value, TextMode::Wrap),
    );
    let cells = if row.bold {
        format!("cells += (text(weight: \"bold\", {label}), text(weight: \"bold\", {value_expr}))")
    } else {
        format!("cells += ({label}, {value_expr})")
    };
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
