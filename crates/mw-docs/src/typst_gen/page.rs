//! Page geometry, the layouts of the copies on the page, the content offset and the cut line.

use std::fmt::Write;

use crate::model::{Layout, Template};

use super::RenderOptions;
use super::blocks::Generator;
use super::code::CodeFile;

/// Space between the two copies of [`Layout::TwoPerA4`]; the cut line runs in its middle.
const CUT_GAP_MM: f64 = 8.0;

/// Page size and margins in mm, the content offset already applied to the margins.
#[derive(Clone, Copy)]
pub(super) struct Page {
    pub(super) width: f64,
    pub(super) height: f64,
    pub(super) top: f64,
    pub(super) right: f64,
    pub(super) bottom: f64,
    pub(super) left: f64,
}

impl Page {
    pub(super) fn new(template: &Template, opts: &RenderOptions) -> Self {
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
pub(super) enum CopyRole {
    /// [`Layout::OnePerPage`]: the only copy, without copy-specific blocks.
    Single,
    Service,
    Client,
}

impl CopyRole {
    /// The value of `copy` in the author's code.
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Service => "service",
            Self::Client => "client",
        }
    }
}

/// The blocks of `template` on the page: one copy, or two with the cut line between them. The code
/// blocks go to `code`.
pub(super) fn body(
    generator: &Generator,
    template: &Template,
    out: &mut String,
    code: &mut Vec<CodeFile>,
) {
    let page = &generator.page;
    let width = generator.area_mm.0;
    match template.layout {
        Layout::OnePerPage => {
            out.push('#');
            out.push_str(&generator.flow(&template.blocks, &[], CopyRole::Single, width, code));
            out.push('\n');
        }
        Layout::SheetPerCopy => {
            copies(generator, template, out, code);
            out.push_str("#copy-service\n#pagebreak()\n#copy-client\n");
        }
        Layout::TwoPerA4 => {
            let half = (generator.area_mm.1 - CUT_GAP_MM) / 2.0;
            copies(generator, template, out, code);
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
}

/// Defines `copy-service` and `copy-client`: the blocks of the template for each copy.
fn copies(generator: &Generator, template: &Template, out: &mut String, code: &mut Vec<CodeFile>) {
    let width = generator.area_mm.0;
    for (name, role) in [
        ("copy-service", CopyRole::Service),
        ("copy-client", CopyRole::Client),
    ] {
        let _ = writeln!(
            out,
            "#let {name} = {}",
            generator.flow(&template.blocks, &[], role, width, code)
        );
    }
}

/// A number for Typst; a non-finite one (a broken template) becomes 0, so the source stays valid.
pub(super) fn number(x: f64) -> String {
    format!("{:.3}", if x.is_finite() { x } else { 0.0 })
}

pub(super) fn mm(x: f64) -> String {
    format!("{}mm", number(x))
}

pub(super) fn pt(x: f64) -> String {
    format!("{}pt", number(x))
}

/// A number for people: at most two decimals, a decimal comma and a real minus (`-1.5` → «−1,5»).
pub(super) fn decimal(x: f64) -> String {
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
