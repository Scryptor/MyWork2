//! Ready-made variants of the acts the workshop can pick in the editor instead of the standard
//! template. "Как в MyWork 2" is the layout of the old program's acts: the two copies side by side on
//! a landscape A4, a framed title, a gridded table and numbered terms.

use mw_domain::documents::DocumentKind;

use crate::model::{Align, Block, BlockBody, FieldRow, Layout, PageSetup, PageSize, Template};
use crate::standard::{both, margins, optional_row, row, text};

const MARGIN_MM: f32 = 8.0;
const BODY_PT: f32 = 8.0;
const FIRM_NAME_PT: f32 = 11.0;
const TITLE_PT: f32 = 11.0;
const SMALL_PT: f32 = 7.0;
const CONSENT_PT: f32 = 7.5;

/// A ready-made variant of a template.
#[derive(Debug, Clone, PartialEq)]
pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub template: Template,
}

/// The variants offered for `kind`: only the acts have any.
pub fn presets(kind: DocumentKind) -> Vec<Preset> {
    match kind {
        DocumentKind::IntakeAct | DocumentKind::BlankIntakeAct => {
            vec![mywork2(kind, "Акт приёма оборудования в ремонт", false)]
        }
        DocumentKind::WarrantyIntakeAct => vec![mywork2(
            kind,
            "Акт приёма оборудования в гарантийный ремонт",
            false,
        )],
        DocumentKind::IssueAct => vec![mywork2(kind, "Акт выдачи оборудования из ремонта", true)],
        DocumentKind::WarrantyIssueAct => vec![mywork2(
            kind,
            "Акт выдачи оборудования после гарантийного ремонта",
            true,
        )],
        DocumentKind::Sticker | DocumentKind::Calibration => Vec::new(),
    }
}

fn mywork2(kind: DocumentKind, title: &str, issue: bool) -> Preset {
    let left = vec![
        both(BlockBody::Logo {
            width_mm: 30.0,
            height_mm: 12.0,
            align: Align::Left,
        }),
        text("{фирма.название}", Some(FIRM_NAME_PT), true, Align::Left),
        text("{фирма.сведения}", Some(SMALL_PT), false, Align::Left),
    ];
    let right = vec![
        both(BlockBody::Qr {
            link: "{ссылка.yandex_maps}".to_owned(),
            size_mm: 16.0,
            caption: String::new(),
            align: Align::Right,
        }),
        both(BlockBody::Barcode {
            height_mm: 9.0,
            digits: true,
            align: Align::Right,
            fill: false,
            stretch: false,
        }),
        both(BlockBody::CopyLabel {
            align: Align::Right,
        }),
    ];
    let (rows, fault, terms, consent, signatures) = if issue {
        (
            issue_rows(),
            "Неисправность: {заказ.неисправность}",
            "{фирма.договор_выдачи}",
            "Оборудование получил(а), претензий к ремонту и внешнему виду не имею.",
            ("Подпись Заказчика", "Подпись Исполнителя"),
        )
    } else {
        (
            intake_rows(kind == DocumentKind::WarrantyIntakeAct),
            "Неисправность со слов клиента: {заказ.неисправность}",
            "{фирма.договор_приёма}",
            "С условиями обслуживания ознакомлен(а) и согласен(на).",
            ("Подпись Заказчика", "Подпись Исполнителя"),
        )
    };
    let blocks: Vec<Block> = vec![
        both(BlockBody::Columns {
            columns: vec![left, right],
            gap_mm: 4.0,
        }),
        text("{фирма.реквизиты}", Some(SMALL_PT), false, Align::Left),
        both(BlockBody::Text {
            text: title.to_owned(),
            size_pt: Some(TITLE_PT),
            bold: true,
            align: Align::Center,
            one_line: false,
            frame: true,
            numbered: false,
        }),
        both(BlockBody::Fields { rows, grid: true }),
        text(fault, None, false, Align::Left),
        text("Условия обслуживания", Some(BODY_PT), true, Align::Left),
        both(BlockBody::Text {
            text: terms.to_owned(),
            size_pt: Some(SMALL_PT),
            bold: false,
            align: Align::Left,
            one_line: false,
            frame: false,
            numbered: true,
        }),
        text(consent, Some(CONSENT_PT), false, Align::Left),
        both(BlockBody::Spacer { mm: 6.0 }),
        both(BlockBody::Signatures {
            left: signatures.0.to_owned(),
            right: signatures.1.to_owned(),
        }),
    ];
    Preset {
        id: "mywork2",
        name: "Как в MyWork 2",
        template: Template {
            kind,
            page: PageSetup {
                size: PageSize::A4,
                landscape: true,
                margins_mm: margins(MARGIN_MM),
            },
            layout: Layout::TwoAcross,
            font_size_pt: BODY_PT,
            blocks,
        },
    }
}

fn intake_rows(warranty: bool) -> Vec<FieldRow> {
    let mut rows = vec![
        row("Номер заказа", "{заказ.номер}"),
        row("Дата приёма", "{заказ.дата_приёма}"),
        row("ФИО клиента", "{клиент.фио}"),
        row("Телефон клиента", "{клиент.телефон}"),
        row("Устройство", "{заказ.устройство}"),
        row("Серийный номер", "{заказ.серийный_номер}"),
        row("Комплектность", "{заказ.комплектность}"),
        row("Внешний вид", "{заказ.состояние}"),
        optional_row("Предоплата", "{заказ.предоплата}"),
    ];
    if warranty {
        rows.push(row("Гарантия", "{заказ.гарантия}"));
    }
    rows.push(row("Предварительная стоимость", "{заказ.оценка}"));
    rows
}

fn issue_rows() -> Vec<FieldRow> {
    vec![
        row("Номер заказа", "{заказ.номер}"),
        row("Дата приёма", "{заказ.дата_приёма}"),
        row("Дата выдачи", "{заказ.дата_выдачи}"),
        row("ФИО клиента", "{клиент.фио}"),
        row("Телефон клиента", "{клиент.телефон}"),
        row("Устройство", "{заказ.устройство}"),
        row("Серийный номер", "{заказ.серийный_номер}"),
        row("Выполненные работы", "{заказ.работы}"),
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
