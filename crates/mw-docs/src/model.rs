//! Template model, also the `.mwtemplate` file format: a page setup and a vertical flow of blocks.
//! A flow, not a canvas: long text pushes the following blocks down instead of overlapping them.

use mw_domain::documents::DocumentKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Template {
    pub kind: DocumentKind,
    pub page: PageSetup,
    pub layout: Layout,
    /// Base text size.
    pub font_size_pt: f32,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PageSetup {
    pub size: PageSize,
    /// Swaps the width and the height of `size`.
    pub landscape: bool,
    pub margins_mm: Margins,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageSize {
    A4,
    A5,
    Custom { width_mm: f32, height_mm: f32 },
}

impl PageSize {
    /// Portrait (width, height) in mm.
    pub fn size_mm(self) -> (f32, f32) {
        match self {
            Self::A4 => (210.0, 297.0),
            Self::A5 => (148.0, 210.0),
            Self::Custom {
                width_mm,
                height_mm,
            } => (width_mm, height_mm),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Margins {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

/// How the copies of a document are laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    /// One copy: blocks for a single copy (`Service`, `Client`) and `CopyLabel` are not printed.
    OnePerPage,
    /// The service copy in the upper half of the sheet, the client copy in the lower half, a dashed cut
    /// line between them. When a copy does not fit into its half, each copy gets its own sheet.
    TwoPerA4,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Block {
    /// The copies the block is printed in.
    pub copies: CopyFilter,
    pub body: BlockBody,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CopyFilter {
    Both,
    Service,
    Client,
}

/// Horizontal alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    Left,
    Center,
    Right,
}

fn align_right() -> Align {
    Align::Right
}

/// Text in `Text`, `FieldRow::value` and `Signatures` may contain substitutions such as `{заказ.номер}`
/// (see [`crate::fields`]); an unknown one is printed as written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BlockBody {
    Text {
        text: String,
        /// `None` — the template's `font_size_pt`.
        size_pt: Option<f32>,
        bold: bool,
        align: Align,
        /// One line that never wraps: line breaks become spaces and what does not fit is cut off at the
        /// right edge (stickers).
        #[serde(default)]
        one_line: bool,
    },
    /// A table «label — value».
    Fields { rows: Vec<FieldRow> },
    /// The order's UPC-A barcode; nothing when the document has no valid code. The module is 0.33 mm
    /// (with printer DPI, a whole number of dots) and shrinks when the code does not fit the width of
    /// the column: down to 2 dots, without DPI down to 0.25 mm.
    Barcode {
        /// Height of the regular bars; the guard bars are 5 modules longer.
        height_mm: f32,
        /// Print the digits under the bars.
        digits: bool,
        align: Align,
        /// In a single-copy document the bars grow to the height the other blocks leave free on the
        /// page, never lower than `height_mm` (one sticker for any label size). Ignored in
        /// [`Layout::TwoPerA4`], where each copy is measured to fit its half.
        #[serde(default)]
        fill: bool,
    },
    /// Two signature lines side by side, each after its caption.
    Signatures { left: String, right: String },
    /// A horizontal line across the column.
    Separator,
    /// Vertical space.
    Spacer { mm: f32 },
    /// Columns of equal width, each a flow of blocks.
    Columns {
        columns: Vec<Vec<Block>>,
        gap_mm: f32,
    },
    /// «Экземпляр сервиса» or «Экземпляр клиента», by the copy.
    CopyLabel {
        #[serde(default = "align_right")]
        align: Align,
    },
    /// A test label for setting up a printer, as tall as the page area: a frame 0.3 mm inside its
    /// edges, a crosshair in the centre and a caption «W × H мм · N dpi · смещение X/Y» from the
    /// render options (page size, printer DPI when given, offset).
    CalibrationMarks,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldRow {
    /// Plain text, printed as written.
    pub label: String,
    /// Text with substitutions.
    pub value: String,
    /// Skip the row when every substitution in `value` is empty (a value without substitutions: when it
    /// is blank). A blank form keeps the row.
    pub hide_if_empty: bool,
}
