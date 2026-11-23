//! Модель шаблона документа — она же формат файла `.mwtemplate`: настройка страницы и вертикальный
//! поток блоков. Поток, а не холст: длинный текст сдвигает следующие блоки вниз, а не налезает на них.

use crate::documents::DocumentKind;
use crate::validation::FieldError;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Template {
    pub kind: DocumentKind,
    pub page: PageSetup,
    pub layout: Layout,
    /// Base text size.
    pub font_size_pt: f32,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PageSetup {
    pub size: PageSize,
    /// Swaps the width and the height of `size`.
    pub landscape: bool,
    pub margins_mm: Margins,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Margins {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

/// How the copies of a document are laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    /// One copy: blocks for a single copy (`Service`, `Client`) and `CopyLabel` are not printed.
    OnePerPage,
    /// The service copy in the upper half of the sheet, the client copy in the lower half, a dashed cut
    /// line between them. When a copy does not fit into its half, each copy gets its own sheet.
    TwoPerA4,
    /// Every copy on its own sheet of the template's page size: the service copy on the first, the
    /// client copy on the next. Not for stickers and calibration.
    SheetPerCopy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Block {
    /// The copies the block is printed in.
    pub copies: CopyFilter,
    pub body: BlockBody,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CopyFilter {
    Both,
    Service,
    Client,
}

/// Horizontal alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
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
/// (список — в модуле `fields` крейта `mw-docs`); an unknown one is printed as written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
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
        /// The module grows until the code fills the width of the column (with DPI by whole printer
        /// dots); never below the nominal 0.33 mm, except on a narrow label, where it shrinks as usual.
        #[serde(default)]
        stretch: bool,
    },
    /// Two signature lines side by side, each after its caption.
    Signatures { left: String, right: String },
    /// A horizontal line across the column.
    Separator,
    /// Vertical space.
    Spacer { mm: f32 },
    /// Columns of equal width, each a flow of blocks.
    Columns {
        #[schema(no_recursion)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct FieldRow {
    /// Plain text, printed as written.
    pub label: String,
    /// Text with substitutions.
    pub value: String,
    /// Skip the row when every substitution in `value` is empty (a value without substitutions: when it
    /// is blank). A blank form keeps the row.
    pub hide_if_empty: bool,
    /// Print the row bold: both the label and the value.
    #[serde(default)]
    pub bold: bool,
}

pub const TEMPLATE_FILE_FORMAT: &str = "mwtemplate";
pub const TEMPLATE_FILE_VERSION: u32 = 1;
pub const TEMPLATE_FILE_MAX_BYTES: usize = 1_048_576;

const MAX_BLOCKS: usize = 200;
const MAX_TEXT_CHARS: usize = 10_000;
const MAX_ROWS: usize = 50;
const MAX_LABEL_CHARS: usize = 200;
const MAX_VALUE_CHARS: usize = 2_000;
const MAX_CAPTION_CHARS: usize = 200;
/// Минимальный размер места под содержимое, мм.
const MIN_CONTENT_MM: f32 = 10.0;

fn in_range(value: f32, min: f32, max: f32) -> bool {
    (min..=max).contains(&value)
}

fn check_range(errors: &mut Vec<FieldError>, field: String, value: f32, min: f32, max: f32) {
    if !in_range(value, min, max) {
        errors.push(FieldError::new(field, "out_of_range"));
    }
}

fn check_len(errors: &mut Vec<FieldError>, field: String, value: &str, max: usize) {
    if value.chars().count() > max {
        errors.push(FieldError::new(field, "too_long"));
    }
}

fn count_blocks(blocks: &[Block]) -> usize {
    blocks
        .iter()
        .map(|b| match &b.body {
            BlockBody::Columns { columns, .. } => {
                1 + columns.iter().map(|c| count_blocks(c)).sum::<usize>()
            }
            _ => 1,
        })
        .sum()
}

impl Template {
    /// Проверяет границы шаблона. Поле ошибки — путь через точку: `blocks.{i}`, во вложенных
    /// `blocks.{i}.columns.{c}.{j}`, затем имя поля.
    pub fn validate(&self) -> Result<(), Vec<FieldError>> {
        let mut errors = Vec::new();
        self.validate_page(&mut errors);
        match self.layout {
            Layout::TwoPerA4 => {
                if self.page.size != PageSize::A4 || self.page.landscape {
                    errors.push(FieldError::new("layout", "requires_a4"));
                }
                if matches!(self.kind, DocumentKind::Sticker | DocumentKind::Calibration) {
                    errors.push(FieldError::new("layout", "not_allowed"));
                }
            }
            Layout::SheetPerCopy => {
                if matches!(self.kind, DocumentKind::Sticker | DocumentKind::Calibration) {
                    errors.push(FieldError::new("layout", "not_allowed"));
                }
            }
            Layout::OnePerPage => {}
        }
        check_range(
            &mut errors,
            "font_size_pt".into(),
            self.font_size_pt,
            5.0,
            36.0,
        );
        if count_blocks(&self.blocks) > MAX_BLOCKS {
            errors.push(FieldError::new("blocks", "too_many"));
        }
        self.validate_blocks(&self.blocks, "blocks", false, &mut errors);
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn validate_page(&self, errors: &mut Vec<FieldError>) {
        let page = &self.page;
        if let PageSize::Custom {
            width_mm,
            height_mm,
        } = page.size
        {
            check_range(errors, "page.size.width_mm".into(), width_mm, 15.0, 420.0);
            check_range(errors, "page.size.height_mm".into(), height_mm, 15.0, 420.0);
        }
        let m = page.margins_mm;
        let mut margins_ok = true;
        for (name, v) in [
            ("top", m.top),
            ("right", m.right),
            ("bottom", m.bottom),
            ("left", m.left),
        ] {
            if !in_range(v, 0.0, 50.0) {
                margins_ok = false;
                errors.push(FieldError::new(
                    format!("page.margins_mm.{name}"),
                    "out_of_range",
                ));
            }
        }
        let (w, h) = page.size.size_mm();
        let (w, h) = if page.landscape { (h, w) } else { (w, h) };
        if margins_ok
            && in_range(w, 15.0, 420.0)
            && in_range(h, 15.0, 420.0)
            && !((w - m.left - m.right) >= MIN_CONTENT_MM
                && (h - m.top - m.bottom) >= MIN_CONTENT_MM)
        {
            errors.push(FieldError::new("page.margins_mm", "too_large"));
        }
    }

    fn validate_blocks(
        &self,
        blocks: &[Block],
        path: &str,
        in_column: bool,
        errors: &mut Vec<FieldError>,
    ) {
        for (i, block) in blocks.iter().enumerate() {
            let at = format!("{path}.{i}");
            match &block.body {
                BlockBody::Text { text, size_pt, .. } => {
                    check_len(errors, format!("{at}.text"), text, MAX_TEXT_CHARS);
                    if let Some(size) = size_pt {
                        check_range(errors, format!("{at}.size_pt"), *size, 5.0, 72.0);
                    }
                }
                BlockBody::Fields { rows } => {
                    if rows.len() > MAX_ROWS {
                        errors.push(FieldError::new(format!("{at}.rows"), "too_many"));
                    }
                    for (k, row) in rows.iter().enumerate() {
                        check_len(
                            errors,
                            format!("{at}.rows.{k}.label"),
                            &row.label,
                            MAX_LABEL_CHARS,
                        );
                        check_len(
                            errors,
                            format!("{at}.rows.{k}.value"),
                            &row.value,
                            MAX_VALUE_CHARS,
                        );
                    }
                }
                BlockBody::Barcode {
                    height_mm, fill, ..
                } => {
                    check_range(errors, format!("{at}.height_mm"), *height_mm, 4.0, 60.0);
                    if *fill && in_column {
                        errors.push(FieldError::new(at.clone(), "fill_in_columns"));
                    }
                }
                BlockBody::Signatures { left, right } => {
                    check_len(errors, format!("{at}.left"), left, MAX_CAPTION_CHARS);
                    check_len(errors, format!("{at}.right"), right, MAX_CAPTION_CHARS);
                }
                BlockBody::Spacer { mm } => {
                    check_range(errors, format!("{at}.mm"), *mm, 0.0, 200.0);
                }
                BlockBody::Columns { columns, gap_mm } => {
                    if in_column {
                        errors.push(FieldError::new(at.clone(), "nested_columns"));
                    } else {
                        if !(2..=4).contains(&columns.len()) {
                            errors.push(FieldError::new(format!("{at}.columns"), "out_of_range"));
                        }
                        check_range(errors, format!("{at}.gap_mm"), *gap_mm, 0.0, 30.0);
                        for (c, column) in columns.iter().enumerate() {
                            self.validate_blocks(
                                column,
                                &format!("{at}.columns.{c}"),
                                true,
                                errors,
                            );
                        }
                    }
                }
                BlockBody::CalibrationMarks => {
                    if self.kind != DocumentKind::Calibration {
                        errors.push(FieldError::new(at.clone(), "not_allowed"));
                    }
                }
                BlockBody::Separator | BlockBody::CopyLabel { .. } => {}
            }
        }
    }
}

/// Файл `.mwtemplate`: шаблон с признаком формата и версией.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TemplateFile {
    pub format: String,
    pub format_version: u32,
    pub template: Template,
}

impl TemplateFile {
    pub fn new(template: Template) -> Self {
        Self {
            format: TEMPLATE_FILE_FORMAT.to_owned(),
            format_version: TEMPLATE_FILE_VERSION,
            template,
        }
    }

    /// Разбирает и проверяет файл; вид документа берётся у вызывающего, а не из файла.
    pub fn from_value(
        value: serde_json::Value,
        kind: DocumentKind,
    ) -> Result<Template, Vec<FieldError>> {
        let invalid = |field: &str| vec![FieldError::new(field, "invalid")];
        let serde_json::Value::Object(mut object) = value else {
            return Err(invalid("file"));
        };
        if object.get("format").and_then(|v| v.as_str()) != Some(TEMPLATE_FILE_FORMAT) {
            return Err(invalid("format"));
        }
        if object
            .keys()
            .any(|k| !matches!(k.as_str(), "format" | "format_version" | "template"))
        {
            return Err(invalid("file"));
        }
        let version = object.get("format_version").and_then(|v| v.as_u64());
        if !version.is_some_and(|v| (1..=u64::from(TEMPLATE_FILE_VERSION)).contains(&v)) {
            return Err(vec![FieldError::new("format_version", "unsupported")]);
        }
        let template = object.remove("template").ok_or_else(|| invalid("file"))?;
        let mut template: Template =
            serde_json::from_value(template).map_err(|_| invalid("file"))?;
        template.kind = kind;
        template.validate()?;
        Ok(template)
    }

    pub fn parse(json: &str, kind: DocumentKind) -> Result<Template, Vec<FieldError>> {
        if json.len() > TEMPLATE_FILE_MAX_BYTES {
            return Err(vec![FieldError::new("file", "too_large")]);
        }
        let value =
            serde_json::from_str(json).map_err(|_| vec![FieldError::new("file", "invalid")])?;
        Self::from_value(value, kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(v: f32) -> Margins {
        Margins {
            top: v,
            right: v,
            bottom: v,
            left: v,
        }
    }

    fn base(kind: DocumentKind) -> Template {
        Template {
            kind,
            page: PageSetup {
                size: PageSize::A4,
                landscape: false,
                margins_mm: m(10.0),
            },
            layout: Layout::OnePerPage,
            font_size_pt: 10.0,
            blocks: vec![],
        }
    }

    fn spacer() -> Block {
        Block {
            copies: CopyFilter::Both,
            body: BlockBody::Spacer { mm: 1.0 },
        }
    }

    fn text(s: String, size: Option<f32>) -> Block {
        Block {
            copies: CopyFilter::Both,
            body: BlockBody::Text {
                text: s,
                size_pt: size,
                bold: false,
                align: Align::Left,
                one_line: false,
            },
        }
    }

    fn columns(cols: Vec<Vec<Block>>) -> Block {
        Block {
            copies: CopyFilter::Both,
            body: BlockBody::Columns {
                columns: cols,
                gap_mm: 2.0,
            },
        }
    }

    fn barcode(fill: bool) -> Block {
        Block {
            copies: CopyFilter::Both,
            body: BlockBody::Barcode {
                height_mm: 10.0,
                digits: true,
                align: Align::Left,
                fill,
                stretch: false,
            },
        }
    }

    fn errs(t: &Template) -> Vec<(String, String)> {
        t.validate()
            .unwrap_err()
            .into_iter()
            .map(|e| (e.field, e.code))
            .collect()
    }

    fn pair(f: &str, c: &str) -> (String, String) {
        (f.to_owned(), c.to_owned())
    }

    #[test]
    fn template_round_trips_through_json() {
        let mut t = base(DocumentKind::IntakeAct);
        t.blocks = vec![text("a".into(), Some(12.0)), barcode(true), spacer()];
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(serde_json::from_str::<Template>(&json).unwrap(), t);
        assert_eq!(t.validate(), Ok(()));
    }

    #[test]
    fn unknown_field_is_rejected() {
        let json =
            r#"{"type":"text","text":"a","size_pt":null,"bold":false,"align":"left","colour":1}"#;
        assert!(serde_json::from_str::<BlockBody>(json).is_err());
    }

    #[test]
    fn two_per_a4_requires_portrait_a4() {
        let mut t = base(DocumentKind::IntakeAct);
        t.layout = Layout::TwoPerA4;
        t.page.size = PageSize::A5;
        assert_eq!(errs(&t), [pair("layout", "requires_a4")]);
        t.page.size = PageSize::A4;
        t.page.landscape = true;
        assert_eq!(errs(&t), [pair("layout", "requires_a4")]);
    }

    #[test]
    fn sticker_allows_only_one_per_page() {
        let mut t = base(DocumentKind::Sticker);
        t.layout = Layout::TwoPerA4;
        assert_eq!(errs(&t), [pair("layout", "not_allowed")]);
    }

    #[test]
    fn calibration_marks_only_in_calibration() {
        let mut t = base(DocumentKind::IntakeAct);
        t.blocks = vec![Block {
            copies: CopyFilter::Both,
            body: BlockBody::CalibrationMarks,
        }];
        assert_eq!(errs(&t), [pair("blocks.0", "not_allowed")]);
        t.kind = DocumentKind::Calibration;
        assert_eq!(t.validate(), Ok(()));
    }

    #[test]
    fn limits_report_their_paths() {
        let mut t = base(DocumentKind::IntakeAct);
        t.blocks = vec![spacer(), text("a".into(), Some(100.0))];
        assert_eq!(errs(&t), [pair("blocks.1.size_pt", "out_of_range")]);
        t.blocks = vec![columns(vec![vec![], vec![text("я".repeat(10_001), None)]])];
        assert_eq!(errs(&t), [pair("blocks.0.columns.1.0.text", "too_long")]);
        t.font_size_pt = f32::NAN;
        t.blocks = vec![];
        assert_eq!(errs(&t), [pair("font_size_pt", "out_of_range")]);
    }

    #[test]
    fn nested_columns_and_fill_in_columns_are_rejected() {
        let mut t = base(DocumentKind::IntakeAct);
        t.blocks = vec![columns(vec![
            vec![columns(vec![vec![], vec![]])],
            vec![spacer(), barcode(true)],
        ])];
        assert_eq!(
            errs(&t),
            [
                pair("blocks.0.columns.0.0", "nested_columns"),
                pair("blocks.0.columns.1.1", "fill_in_columns"),
            ]
        );
        t.blocks = vec![columns(vec![vec![]])];
        assert_eq!(errs(&t), [pair("blocks.0.columns", "out_of_range")]);
    }

    #[test]
    fn too_many_blocks() {
        let mut t = base(DocumentKind::IntakeAct);
        t.blocks = (0..201).map(|_| spacer()).collect();
        assert_eq!(errs(&t), [pair("blocks", "too_many")]);
        t.blocks.truncate(200);
        assert_eq!(t.validate(), Ok(()));
    }

    #[test]
    fn margins_must_leave_room() {
        let mut t = base(DocumentKind::Sticker);
        t.page.size = PageSize::Custom {
            width_mm: 20.0,
            height_mm: 20.0,
        };
        t.page.margins_mm = m(6.0);
        assert_eq!(errs(&t), [pair("page.margins_mm", "too_large")]);
        t.page.size = PageSize::Custom {
            width_mm: 10.0,
            height_mm: 20.0,
        };
        t.page.margins_mm = m(1.0);
        assert_eq!(errs(&t), [pair("page.size.width_mm", "out_of_range")]);
    }

    fn file(format: &str, version: u32, t: &Template) -> String {
        let mut f = TemplateFile::new(t.clone());
        f.format = format.to_owned();
        f.format_version = version;
        serde_json::to_string(&f).unwrap()
    }

    #[test]
    fn file_checks_format_version_and_kind() {
        let t = base(DocumentKind::IntakeAct);
        let kind = DocumentKind::IssueAct;
        let code = |json: &str| {
            let e = TemplateFile::parse(json, kind).unwrap_err();
            assert_eq!(e.len(), 1);
            (e[0].field.clone(), e[0].code.clone())
        };
        assert_eq!(code(&file("x", 1, &t)), pair("format", "invalid"));
        assert_eq!(
            code(&file("mwtemplate", 0, &t)),
            pair("format_version", "unsupported")
        );
        assert_eq!(
            code(&file("mwtemplate", 2, &t)),
            pair("format_version", "unsupported")
        );
        assert_eq!(code("{"), pair("file", "invalid"));
        assert_eq!(code("[]"), pair("file", "invalid"));
        assert_eq!(
            code(&format!(
                r#"{{"format":"mwtemplate","format_version":1,"template":{{}},"x":"{}"}}"#,
                "a".repeat(TEMPLATE_FILE_MAX_BYTES)
            )),
            pair("file", "too_large")
        );
        let parsed = TemplateFile::parse(&file("mwtemplate", 1, &t), kind).unwrap();
        assert_eq!(parsed.kind, DocumentKind::IssueAct);
    }

    #[test]
    fn sheet_per_copy_is_not_for_stickers() {
        let mut t = base(DocumentKind::Sticker);
        t.layout = Layout::SheetPerCopy;
        assert_eq!(errs(&t), [pair("layout", "not_allowed")]);
        t.kind = DocumentKind::Calibration;
        assert_eq!(errs(&t), [pair("layout", "not_allowed")]);
        let mut t = base(DocumentKind::IntakeAct);
        t.layout = Layout::SheetPerCopy;
        t.page.size = PageSize::A5;
        assert_eq!(t.validate(), Ok(()));
    }

    #[test]
    fn old_json_without_new_fields_still_reads() {
        let row: FieldRow =
            serde_json::from_str(r#"{"label":"a","value":"b","hide_if_empty":true}"#).unwrap();
        assert!(!row.bold);
        let body: BlockBody = serde_json::from_str(
            r#"{"type":"barcode","height_mm":10.0,"digits":true,"align":"left"}"#,
        )
        .unwrap();
        assert!(matches!(
            body,
            BlockBody::Barcode {
                stretch: false,
                fill: false,
                ..
            }
        ));
        assert_eq!(
            serde_json::to_value(Layout::SheetPerCopy).unwrap(),
            "sheet_per_copy"
        );
    }

    #[test]
    fn file_rejects_unknown_top_level_keys() {
        let t = base(DocumentKind::IntakeAct);
        let mut value: serde_json::Value =
            serde_json::from_str(&file("mwtemplate", 1, &t)).unwrap();
        value["extra"] = 1.into();
        let e = TemplateFile::from_value(value, DocumentKind::IntakeAct).unwrap_err();
        assert_eq!(
            e.into_iter().map(|e| (e.field, e.code)).collect::<Vec<_>>(),
            [pair("file", "invalid")]
        );
    }

    #[test]
    fn custom_page_size_rejects_unknown_fields() {
        let ok: PageSize =
            serde_json::from_str(r#"{"custom":{"width_mm":50.0,"height_mm":30.0}}"#).unwrap();
        assert_eq!(
            ok,
            PageSize::Custom {
                width_mm: 50.0,
                height_mm: 30.0
            }
        );
        assert!(
            serde_json::from_str::<PageSize>(
                r#"{"custom":{"width_mm":50.0,"height_mm":30.0,"x":1}}"#
            )
            .is_err()
        );
    }

    #[test]
    fn text_limit_counts_chars_not_bytes() {
        let mut t = base(DocumentKind::IntakeAct);
        t.blocks = vec![text("ж".repeat(10_000), None)];
        assert_eq!(t.validate(), Ok(()));
    }

    #[test]
    fn content_area_is_checked_after_landscape_swap() {
        let mut t = base(DocumentKind::Sticker);
        t.page.size = PageSize::Custom {
            width_mm: 100.0,
            height_mm: 20.0,
        };
        t.page.margins_mm = Margins {
            top: 4.0,
            right: 40.0,
            bottom: 4.0,
            left: 40.0,
        };
        assert_eq!(t.validate(), Ok(()));
        t.page.landscape = true;
        assert_eq!(errs(&t), [pair("page.margins_mm", "too_large")]);
    }
}
