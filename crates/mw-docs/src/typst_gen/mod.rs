//! Template → Typst source.
//!
//! User data never becomes markup. [`to_typst`] sees only the template and the options; the values are read
//! inside the document from `data.json`, and template text goes in as escaped Typst string literals.

mod blocks;
mod code;
mod page;
mod qr;
mod text;

use std::collections::BTreeMap;
use std::fmt::Write;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::barcode::{Shape, barcode_svg};
use crate::fields::fields;
use crate::fonts::{FontSet, shared};
use crate::model::{Block, BlockBody, Template};
use crate::render::{DocsError, Rendered, compile};

use blocks::Generator;
use code::CodeBlocks;
use page::{body, mm, pt};
use qr::qr_files;
use text::typst_str;

pub(crate) use code::CodeSources;
pub use code::block_code;

/// The values of one document. In JSON without the logo: a [`crate::Job`] carries it once for all
/// its documents.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentData {
    /// By field key (`order.number`, see [`crate::fields`]); a missing key is an empty value.
    pub values: BTreeMap<String, String>,
    /// The order's barcode: UPC-A digits or, for old orders, up to 40 printable ASCII characters
    /// (printed as Code 128). `None` or a value that cannot be encoded prints no barcode.
    pub barcode: Option<String>,
    /// The firm's logo, a PNG. `None` prints nothing in the Logo blocks.
    #[serde(skip)]
    pub logo: Option<Arc<[u8]>>,
    /// A form to fill in by hand: empty values become lines to write on.
    pub blank: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct RenderOptions {
    /// Label (width, height) in mm; replaces the template's page size (stickers, calibration).
    pub label_mm: Option<(f32, f32)>,
    /// Shift of the page content in mm: x to the right, y down.
    pub offset_mm: (f32, f32),
    /// Printer resolution: the barcode module becomes a whole number of printer dots. `None` — 0.33 mm.
    pub dpi: Option<u32>,
}

/// Compiles `template` with `data` on the shared set of fonts.
pub fn render(
    template: &Template,
    data: &DocumentData,
    opts: &RenderOptions,
) -> Result<Rendered, DocsError> {
    render_with_fonts(template, data, opts, shared())
}

/// Compiles `template` with `data` on the given set of fonts.
pub fn render_with_fonts(
    template: &Template,
    data: &DocumentData,
    opts: &RenderOptions,
    fonts: &FontSet,
) -> Result<Rendered, DocsError> {
    render_with_logo(template, data, data.logo.as_deref(), opts, fonts)
}

/// Like [`render_with_fonts`] with `logo` in place of `data.logo`.
pub(crate) fn render_with_logo(
    template: &Template,
    data: &DocumentData,
    logo: Option<&[u8]>,
    opts: &RenderOptions,
    fonts: &FontSet,
) -> Result<Rendered, DocsError> {
    // `barcode` in `data.json` is set exactly when `barcode.svg` exists. Its width in modules depends on
    // the value, so it goes to the generator as a number; the value itself never enters the markup.
    let coded = data
        .barcode
        .as_deref()
        .and_then(|code| Shape::of(code).map(|shape| (code, shape)));
    let shape = coded.map_or(Shape::UPC_A, |(_, shape)| shape);
    let barcode = coded
        .zip(Generator::new(template, opts, shape).svg)
        .and_then(|((code, _), (module, bars))| {
            barcode_svg(code, module, bars).map(|svg| (code, svg))
        });
    // Likewise `logo` in `data.json` is set exactly when `logo.png` exists. The author's code may
    // show it too.
    let logo = logo.filter(|_| has_logo_block(&template.blocks) || template.has_code());
    // And a name is in `qr` exactly when `qr-<name>.svg` exists; the link is only in the file.
    let qr = qr_files(template, &data.values);
    let qr_names: Vec<&str> = qr.iter().map(|(name, _)| name.as_str()).collect();
    let mut files = vec![(
        "data.json".to_owned(),
        data_json(
            data,
            barcode.as_ref().map(|(code, _)| *code),
            logo.is_some(),
            &qr_names,
        ),
    )];
    if let Some(png) = logo {
        files.push(("logo.png".to_owned(), png.to_vec()));
    }
    if let Some((_, svg)) = barcode {
        files.push(("barcode.svg".to_owned(), svg.into_bytes()));
    }
    for (name, svg) in qr {
        files.push((format!("qr-{name}.svg"), svg.into_bytes()));
    }
    let (main, code) = typst_source(template, opts, shape);
    compile(main, code, files, fonts)
}

fn data_json(data: &DocumentData, barcode: Option<&str>, logo: bool, qr: &[&str]) -> Vec<u8> {
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
    root.insert("logo".to_owned(), Value::Bool(logo));
    root.insert(
        "qr".to_owned(),
        Value::Array(
            qr.iter()
                .map(|name| Value::String((*name).to_owned()))
                .collect(),
        ),
    );
    root.insert("blank".to_owned(), Value::Bool(data.blank));
    Value::Object(root).to_string().into_bytes()
}

/// Whether `blocks`, columns included, hold a Logo block.
fn has_logo_block(blocks: &[Block]) -> bool {
    blocks.iter().any(|block| match &block.body {
        BlockBody::Logo { .. } => true,
        BlockBody::Columns { columns, .. } => columns.iter().any(|c| has_logo_block(c)),
        _ => false,
    })
}

/// Helpers every document starts with (after `empties`, which `v` captures); the author's code gets
/// them from `/mw.typ`.
/// `v(key)`: a non-empty value with its line breaks; empty — in a blank form a line to write on for
/// the order and the client fields (the firm's own fields stay empty), otherwise the field's empty
/// text. The line runs to the end of the text line; `line: short-line` — a short one, for centred
/// text. `v1(key)`: the same in one line, with a short line. `one-line(where, body)`: `body` without
/// wrapping; a string that is too long is cut to the longest start (by grapheme clusters, found by
/// bisection) that fits together with «…» (U+2026); other content is cut off at the right edge (the
/// outset keeps accents and descenders inside the clip). `qr-image(name, width)`: the QR code named
/// `name` (see [`qr::qr_name`]) `width` wide; nothing when the document has none.
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
  let line(content) = block(width: 100%, clip: true, outset: (top: 0.3em, bottom: 0.3em), content)
  if width <= size.width { line(align(where, body)) } else if type(body) == str {
    let clusters = body.clusters()
    let (fit, over) = (0, clusters.len())
    while fit < over {
      let mid = calc.div-euclid(fit + over + 1, 2)
      if measure(clusters.slice(0, mid).join(default: "") + "…").width <= size.width { fit = mid } else { over = mid - 1 }
    }
    line(align(where, clusters.slice(0, fit).join(default: "").trim(at: end) + "…"))
  } else { line(box(width: width + 1pt, body)) }
})
#let qr-image(name, width) = if name in data.qr { image("qr-" + name + ".svg", width: width) }
"#;

/// Typst source of `template`. It never contains document data, so it is the same for every order of
/// one barcode width; this one takes a UPC-A barcode. The author's code blocks are included from
/// sources of their own, which this does not return.
pub fn to_typst(template: &Template, opts: &RenderOptions) -> String {
    typst_source(template, opts, Shape::UPC_A).0
}

/// The main source of `template` and, when it has code blocks, the sources they need.
fn typst_source(
    template: &Template,
    opts: &RenderOptions,
    shape: Shape,
) -> (String, Option<CodeSources>) {
    let generator = Generator::new(template, opts, shape);
    let page = &generator.page;
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
    let helpers = helpers();
    out.push_str(&helpers);
    let mut code = CodeBlocks::Files(Vec::new());
    body(&generator, template, &mut out, &mut code);
    let code = template
        .has_code()
        .then(|| CodeSources::new(helpers, code.into_files()));
    (out, code)
}

/// `empties` and [`PRELUDE`]: the start of every document and the text of `/mw.typ`.
fn helpers() -> String {
    let empties: Vec<String> = fields()
        .iter()
        .filter(|f| !f.empty.is_empty())
        .map(|f| format!("{}: {}", typst_str(f.key), typst_str(f.empty)))
        .collect();
    let mut out = if empties.is_empty() {
        "#let empties = (:)\n".to_owned()
    } else {
        format!("#let empties = ({},)\n", empties.join(", "))
    };
    out.push_str(PRELUDE);
    out
}
