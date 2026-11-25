#![allow(clippy::unwrap_used, clippy::expect_used)]

use mw_docs::model::{BlockBody, Layout, PageSize};
use mw_docs::{RenderOptions, presets, render, sample_data};
use mw_domain::documents::DocumentKind;

const ACTS: [DocumentKind; 5] = [
    DocumentKind::IntakeAct,
    DocumentKind::WarrantyIntakeAct,
    DocumentKind::BlankIntakeAct,
    DocumentKind::IssueAct,
    DocumentKind::WarrantyIssueAct,
];

fn title(kind: DocumentKind) -> &'static str {
    match kind {
        DocumentKind::IntakeAct | DocumentKind::BlankIntakeAct => {
            "Акт приёма оборудования в ремонт"
        }
        DocumentKind::WarrantyIntakeAct => "Акт приёма оборудования в гарантийный ремонт",
        DocumentKind::IssueAct => "Акт выдачи оборудования из ремонта",
        _ => "Акт выдачи оборудования после гарантийного ремонта",
    }
}

/// The starts of the sample terms of `kind`, in order: each is printed as a numbered item.
fn terms_starts(kind: DocumentKind) -> &'static [&'static str] {
    match kind {
        DocumentKind::IssueAct | DocumentKind::WarrantyIssueAct => {
            &["Устройство получено", "Гарантия распространяется"]
        }
        _ => &[
            "Устройство принято",
            "Срок диагностики",
            "Сервис не отвечает",
        ],
    }
}

fn mywork2(kind: DocumentKind) -> mw_docs::Preset {
    let list = presets(kind);
    assert_eq!(list.len(), 1, "{kind:?}");
    let preset = list.into_iter().next().unwrap();
    assert_eq!(preset.id, "mywork2");
    assert_eq!(preset.name, "Как в MyWork 2");
    preset
}

#[test]
fn mywork2_presets_validate() {
    for kind in ACTS {
        let t = mywork2(kind).template;
        assert_eq!(t.kind, kind);
        assert_eq!(t.validate(), Ok(()), "{kind:?}");
        assert_eq!(t.layout, Layout::TwoAcross);
        assert_eq!(t.page.size, PageSize::A4);
        assert!(t.page.landscape);
    }
    for kind in [DocumentKind::Sticker, DocumentKind::Calibration] {
        assert!(presets(kind).is_empty(), "{kind:?}");
    }
}

#[test]
fn mywork2_presets_fit_one_landscape_sheet() {
    for kind in ACTS {
        let t = mywork2(kind).template;
        let doc = render(&t, &sample_data(kind), &RenderOptions::default())
            .unwrap_or_else(|e| panic!("{kind:?}: {e}"));
        let text = doc.text();
        assert_eq!(doc.page_count(), 1, "{kind:?}: {text:?}");
        let (w, h) = doc.page_size_mm();
        assert!(w > h, "{w}x{h}");
        assert!(text.contains("Подпись Заказчика"), "{kind:?}: {text:?}");
        assert_eq!(
            text.matches("Подпись Заказчика").count(),
            2,
            "{kind:?}: оба экземпляра"
        );
        let flat = text.replace('\n', "");
        for (n, start) in terms_starts(kind).iter().enumerate() {
            let item = format!("{}.{start}", n + 1);
            assert_eq!(
                flat.matches(&item).count(),
                2,
                "{kind:?}: {item} в {flat:?}"
            );
        }
        assert!(text.contains(title(kind)), "{kind:?}: {text:?}");
        assert!(text.contains("Условия обслуживания"), "{kind:?}");
        if matches!(
            kind,
            DocumentKind::IssueAct | DocumentKind::WarrantyIssueAct
        ) {
            assert!(text.contains("К оплате"), "{kind:?}: {text:?}");
        }
    }
}

#[test]
fn mywork2_presets_print_the_firm_phone_in_the_header() {
    for kind in ACTS {
        let t = mywork2(kind).template;
        let data = sample_data(kind);
        let phone = data.values.get("firm.phone").expect("образец телефона");
        assert!(!phone.is_empty());
        let doc = render(&t, &data, &RenderOptions::default())
            .unwrap_or_else(|e| panic!("{kind:?}: {e}"));
        assert_eq!(doc.page_count(), 1, "{kind:?}");
        let flat = doc.text().replace('\n', "");
        assert_eq!(
            flat.matches(phone.as_str()).count(),
            2,
            "{kind:?}: {flat:?}"
        );
    }
}

fn labels(kind: DocumentKind) -> Vec<String> {
    let t = mywork2(kind).template;
    t.blocks
        .iter()
        .find_map(|b| match &b.body {
            BlockBody::Fields { rows, grid } => {
                assert!(grid);
                Some(rows.iter().map(|r| r.label.clone()).collect())
            }
            _ => None,
        })
        .expect("таблица")
}

#[test]
fn mywork2_preset_has_the_rows_of_the_spec() {
    let intake = [
        "Номер заказа",
        "Дата приёма",
        "ФИО клиента",
        "Телефон клиента",
        "Устройство",
        "Серийный номер",
        "Комплектность",
        "Внешний вид",
        "Предоплата",
        "Предварительная стоимость",
    ];
    let mut warranty: Vec<&str> = intake.to_vec();
    warranty.insert(9, "Гарантия");
    let issue = [
        "Номер заказа",
        "Дата приёма",
        "Дата выдачи",
        "ФИО клиента",
        "Телефон клиента",
        "Устройство",
        "Серийный номер",
        "Выполненные работы",
        "Гарантия",
        "Стоимость",
        "Скидка",
        "Предоплата",
        "К оплате",
    ];
    for (kind, want) in [
        (DocumentKind::IntakeAct, intake.to_vec()),
        (DocumentKind::BlankIntakeAct, intake.to_vec()),
        (DocumentKind::WarrantyIntakeAct, warranty),
        (DocumentKind::IssueAct, issue.to_vec()),
        (DocumentKind::WarrantyIssueAct, issue.to_vec()),
    ] {
        assert_eq!(labels(kind), want, "{kind:?}");
    }
}
