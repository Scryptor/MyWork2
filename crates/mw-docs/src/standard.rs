//! The standard templates every workshop starts with, and the sample data their previews are printed on.
//!
//! Acts are two copies on an A4 sheet: the firm and the barcode on top, the title, a table of the order,
//! the terms, signatures and the legal details. The sticker is the order's barcode over four one-line
//! texts; the calibration label is a frame and a crosshair for setting up the label printer.

use mw_domain::documents::DocumentKind;

use crate::fields::fields;
use crate::model::{
    Align, Block, BlockBody, CopyFilter, FieldRow, Layout, Margins, PageSetup, PageSize, Template,
};
use crate::typst_gen::DocumentData;

const ACT_MARGIN_MM: f32 = 10.0;
/// The logo's box in the header of the acts.
const LOGO_WIDTH_MM: f32 = 40.0;
const LOGO_HEIGHT_MM: f32 = 15.0;
const BODY_PT: f32 = 9.0;
const FIRM_NAME_PT: f32 = 13.0;
const TITLE_PT: f32 = 11.0;
const DETAILS_PT: f32 = 8.0;
const TERMS_PT: f32 = 7.5;
const LEGAL_PT: f32 = 7.0;

/// The default label of the sticker and the calibration label; the print settings may replace it.
const LABEL_MM: (f32, f32) = (58.0, 40.0);
const STICKER_MARGIN_MM: f32 = 1.5;
const STICKER_PT: f32 = 8.0;
const STICKER_NUMBER_PT: f32 = 9.0;

/// The standard template of `kind`.
pub fn standard_template(kind: DocumentKind) -> Template {
    match kind {
        DocumentKind::IntakeAct | DocumentKind::BlankIntakeAct => act(
            kind,
            "Акт приёма № {заказ.номер} от {заказ.дата_приёма}",
            intake_rows(false),
            "{фирма.договор_приёма}",
            ("Сдал", "Принял"),
        ),
        DocumentKind::WarrantyIntakeAct => act(
            kind,
            "Акт приёма по гарантии № {заказ.номер} от {заказ.дата_приёма}",
            intake_rows(true),
            "{фирма.договор_приёма}",
            ("Сдал", "Принял"),
        ),
        DocumentKind::IssueAct => act(
            kind,
            "Акт выдачи № {заказ.номер} от {заказ.дата_выдачи}",
            issue_rows(),
            "{фирма.договор_выдачи}",
            ("Получил", "Выдал"),
        ),
        DocumentKind::WarrantyIssueAct => act(
            kind,
            "Акт выдачи после гарантийного ремонта № {заказ.номер} от {заказ.дата_выдачи}",
            issue_rows(),
            "{фирма.договор_выдачи}",
            ("Получил", "Выдал"),
        ),
        DocumentKind::Sticker => sticker(),
        DocumentKind::Calibration => calibration(),
    }
}

/// The data the sample of `kind` is printed on: every field's sample and the barcode of order 1234. A
/// blank act gets only the firm's own fields (the rest are lines to write on), the calibration label
/// nothing.
pub fn sample_data(kind: DocumentKind) -> DocumentData {
    let samples = || {
        fields()
            .iter()
            .map(|f| (f.key.to_owned(), f.sample.to_owned()))
    };
    match kind {
        DocumentKind::Calibration => DocumentData::default(),
        DocumentKind::BlankIntakeAct => DocumentData {
            values: samples()
                .filter(|(key, _)| !key.starts_with("order.") && !key.starts_with("client."))
                .collect(),
            barcode: None,
            logo: None,
            blank: true,
        },
        _ => {
            let values: std::collections::BTreeMap<_, _> = samples().collect();
            DocumentData {
                barcode: values.get("order.barcode").cloned(),
                values,
                logo: None,
                blank: false,
            }
        }
    }
}

fn both(body: BlockBody) -> Block {
    Block {
        copies: CopyFilter::Both,
        body,
    }
}

fn text(text: &str, size_pt: Option<f32>, bold: bool, align: Align) -> Block {
    both(BlockBody::Text {
        text: text.to_owned(),
        size_pt,
        bold,
        align,
        one_line: false,
    })
}

fn row(label: &str, value: &str) -> FieldRow {
    FieldRow {
        label: label.to_owned(),
        value: value.to_owned(),
        hide_if_empty: false,
        bold: false,
    }
}

fn optional_row(label: &str, value: &str) -> FieldRow {
    FieldRow {
        hide_if_empty: true,
        ..row(label, value)
    }
}

fn act(
    kind: DocumentKind,
    title: &str,
    rows: Vec<FieldRow>,
    terms: &str,
    (left, right): (&str, &str),
) -> Template {
    let firm = vec![
        both(BlockBody::Logo {
            width_mm: LOGO_WIDTH_MM,
            height_mm: LOGO_HEIGHT_MM,
            align: Align::Left,
        }),
        text("{фирма.название}", Some(FIRM_NAME_PT), true, Align::Left),
        text("{фирма.телефон}", None, false, Align::Left),
        text("{фирма.сведения}", Some(DETAILS_PT), false, Align::Left),
    ];
    let code = vec![
        both(BlockBody::Barcode {
            height_mm: 12.0,
            digits: true,
            align: Align::Right,
            fill: false,
            stretch: false,
        }),
        both(BlockBody::CopyLabel {
            align: Align::Right,
        }),
    ];
    Template {
        kind,
        page: PageSetup {
            size: PageSize::A4,
            landscape: false,
            margins_mm: margins(ACT_MARGIN_MM),
        },
        layout: Layout::TwoPerA4,
        font_size_pt: BODY_PT,
        blocks: vec![
            both(BlockBody::Columns {
                columns: vec![firm, code],
                gap_mm: 6.0,
            }),
            both(BlockBody::Separator {}),
            text(title, Some(TITLE_PT), true, Align::Center),
            both(BlockBody::Fields { rows }),
            text(terms, Some(TERMS_PT), false, Align::Left),
            // Room to sign above the lines.
            both(BlockBody::Spacer { mm: 7.0 }),
            both(BlockBody::Signatures {
                left: left.to_owned(),
                right: right.to_owned(),
            }),
            both(BlockBody::Spacer { mm: 1.0 }),
            text("{фирма.реквизиты}", Some(LEGAL_PT), false, Align::Left),
        ],
    }
}

/// The table of an intake act; a warranty intake also names the warranty.
fn intake_rows(warranty: bool) -> Vec<FieldRow> {
    let mut rows = vec![
        row("Клиент", "{клиент.фио}"),
        optional_row("Телефон", "{клиент.телефон}"),
        row("Адрес", "{клиент.адрес}"),
        row("Устройство", "{заказ.устройство}"),
        row("Серийный номер", "{заказ.серийный_номер}"),
        optional_row("Цвет", "{заказ.цвет}"),
        row("Комплектность", "{заказ.комплектность}"),
        row("Внешний вид", "{заказ.состояние}"),
        row("Неисправность", "{заказ.неисправность}"),
    ];
    if warranty {
        rows.push(row("Гарантия", "{заказ.гарантия}"));
    }
    rows.extend([
        row("Предварительная стоимость", "{заказ.оценка}"),
        optional_row("Предоплата", "{заказ.предоплата}"),
    ]);
    rows
}

fn issue_rows() -> Vec<FieldRow> {
    vec![
        row("Клиент", "{клиент.фио}"),
        optional_row("Телефон", "{клиент.телефон}"),
        row("Устройство", "{заказ.устройство}"),
        row("Серийный номер", "{заказ.серийный_номер}"),
        row("Неисправность", "{заказ.неисправность}"),
        row("Выполненные работы", "{заказ.работы}"),
        optional_row("Мастер", "{заказ.мастер}"),
        row("Гарантия", "{заказ.гарантия}"),
        row("Стоимость", "{заказ.стоимость}"),
        optional_row("Скидка", "{заказ.скидка}"),
        optional_row("Предоплата", "{заказ.предоплата}"),
        FieldRow {
            bold: true,
            ..row("К оплате", "{заказ.к_оплате}")
        },
    ]
}

/// The barcode across the label, as tall as the four lines under it leave room for (about 45 % of a
/// 58×40 label), so that one template serves every label size.
fn sticker() -> Template {
    let line = |text: &str, size_pt: Option<f32>| {
        both(BlockBody::Text {
            text: text.to_owned(),
            size_pt,
            bold: true,
            align: Align::Left,
            one_line: true,
        })
    };
    Template {
        kind: DocumentKind::Sticker,
        page: label_page(STICKER_MARGIN_MM),
        layout: Layout::OnePerPage,
        font_size_pt: STICKER_PT,
        blocks: vec![
            both(BlockBody::Barcode {
                height_mm: 6.0,
                digits: true,
                align: Align::Center,
                fill: true,
                stretch: false,
            }),
            line(
                "№ {заказ.номер} · {заказ.дата_приёма}",
                Some(STICKER_NUMBER_PT),
            ),
            line("{клиент.фио}", None),
            line("{клиент.телефон}", None),
            line("{заказ.устройство}", None),
        ],
    }
}

/// No margins: the frame is drawn 0.3 mm inside the label's edges, moved by the print offset.
fn calibration() -> Template {
    Template {
        kind: DocumentKind::Calibration,
        page: label_page(0.0),
        layout: Layout::OnePerPage,
        font_size_pt: LEGAL_PT,
        blocks: vec![both(BlockBody::CalibrationMarks {})],
    }
}

fn label_page(margin_mm: f32) -> PageSetup {
    PageSetup {
        size: PageSize::Custom {
            width_mm: LABEL_MM.0,
            height_mm: LABEL_MM.1,
        },
        landscape: false,
        margins_mm: margins(margin_mm),
    }
}

fn margins(mm: f32) -> Margins {
    Margins {
        top: mm,
        right: mm,
        bottom: mm,
        left: mm,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_templates_are_valid() {
        for kind in DocumentKind::ALL {
            assert_eq!(standard_template(kind).validate(), Ok(()), "{kind:?}");
        }
    }
}
