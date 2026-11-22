use std::collections::BTreeMap;

use mw_docs::model::{
    Align, Block, BlockBody, CopyFilter, FieldRow, Layout, Margins, PageSetup, PageSize, Template,
};
use mw_docs::{DocumentData, RenderOptions, Rendered, render};
use mw_domain::documents::DocumentKind;

/// Typst markup, math, comments, labels and references: all of it must print as plain text.
const SPECIAL: [&str; 12] = [
    "#set page(width: 1mm)",
    "$x$",
    "*жирно*",
    "_a_",
    "[",
    "]",
    "\\",
    "\"",
    "<a>",
    "@ref",
    "// к",
    "/*",
];

fn block(copies: CopyFilter, body: BlockBody) -> Block {
    Block { copies, body }
}

fn text(text: &str) -> Block {
    block(
        CopyFilter::Both,
        BlockBody::Text {
            text: text.to_owned(),
            size_pt: None,
            bold: false,
            align: Align::Left,
        },
    )
}

fn row(label: &str, value: &str, hide_if_empty: bool) -> FieldRow {
    FieldRow {
        label: label.to_owned(),
        value: value.to_owned(),
        hide_if_empty,
    }
}

fn fields(rows: Vec<FieldRow>) -> Block {
    block(CopyFilter::Both, BlockBody::Fields { rows })
}

fn template(layout: Layout, blocks: Vec<Block>) -> Template {
    Template {
        kind: DocumentKind::IntakeAct,
        page: PageSetup {
            size: PageSize::A4,
            landscape: false,
            margins_mm: Margins {
                top: 10.0,
                right: 10.0,
                bottom: 10.0,
                left: 10.0,
            },
        },
        layout,
        font_size_pt: 9.0,
        blocks,
    }
}

fn data(values: &[(&str, &str)]) -> DocumentData {
    DocumentData {
        values: values
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect::<BTreeMap<_, _>>(),
        barcode: None,
        blank: false,
    }
}

fn rendered(template: &Template, data: &DocumentData) -> Rendered {
    render(template, data, &RenderOptions::default()).unwrap_or_else(|e| panic!("renders: {e}"))
}

fn assert_a4(doc: &Rendered) {
    let (w, h) = doc.page_size_mm();
    assert!(
        (w - 210.0).abs() < 0.01 && (h - 297.0).abs() < 0.01,
        "{w}x{h}"
    );
}

#[test]
fn special_characters_in_data_are_printed_verbatim() {
    let t = template(
        Layout::OnePerPage,
        vec![
            text("{заказ.неисправность}"),
            fields(vec![row("Клиент", "{клиент.фио}", false)]),
        ],
    );
    let all = SPECIAL.join(" ");
    let d = data(&[
        ("order.malfunction", &SPECIAL.join("\n")),
        ("client.name", &all),
    ]);
    let doc = rendered(&t, &d);
    let out = doc.text();
    assert!(out.contains(&SPECIAL.join("\n")), "text was {out:?}");
    assert!(out.contains(&all), "text was {out:?}");
    assert_eq!(doc.page_count(), 1);
    assert_a4(&doc);
}

#[test]
fn special_characters_in_template_text_are_printed_verbatim() {
    let t = template(
        Layout::OnePerPage,
        vec![
            text(&SPECIAL.join("\n")),
            fields(vec![row(&SPECIAL.join(" "), "{заказ.номер}", false)]),
            block(
                CopyFilter::Both,
                BlockBody::Signatures {
                    left: SPECIAL[0].to_owned(),
                    right: SPECIAL[1].to_owned(),
                },
            ),
        ],
    );
    let doc = rendered(&t, &data(&[("order.number", "7")]));
    let out = doc.text();
    assert!(out.contains(&SPECIAL.join("\n")), "text was {out:?}");
    assert!(out.contains(&SPECIAL.join(" ")), "text was {out:?}");
    assert_eq!(doc.page_count(), 1);
    assert_a4(&doc);
}

#[test]
fn placeholders_match_ignoring_case_and_yo_unknown_ones_stay() {
    let t = template(
        Layout::OnePerPage,
        vec![text(
            "{заказ.номер}|{ЗАКАЗ.НОМЕР}|{заказ.дата_приема}|{заказ.нет_такого}|{{заказ.номер}}",
        )],
    );
    let d = data(&[("order.number", "42"), ("order.received", "22 ноября 2026")]);
    let out = rendered(&t, &d).text();
    assert!(
        out.contains("42|42|22 ноября 2026|{заказ.нет_такого}|{42}"),
        "text was {out:?}"
    );
}

#[test]
fn multiline_value_keeps_its_lines() {
    let t = template(
        Layout::OnePerPage,
        vec![
            text("{заказ.неисправность}"),
            fields(vec![row("Работы", "{заказ.работы}", false)]),
        ],
    );
    let d = data(&[
        (
            "order.malfunction",
            "Не включается\nРазбит экран\nНе заряжается",
        ),
        ("order.work_done", "Замена экрана\r\nЧистка"),
    ]);
    let out = rendered(&t, &d).text();
    assert!(
        out.contains("Не включается\nРазбит экран\nНе заряжается"),
        "text was {out:?}"
    );
    assert!(out.contains("Замена экрана\nЧистка"), "text was {out:?}");
}

fn act_with_copies() -> Template {
    template(
        Layout::TwoPerA4,
        vec![
            block(CopyFilter::Both, BlockBody::CopyLabel),
            text("Акт приёма № {заказ.номер}"),
            block(
                CopyFilter::Service,
                BlockBody::Text {
                    text: "Только для сервиса".to_owned(),
                    size_pt: None,
                    bold: true,
                    align: Align::Right,
                },
            ),
            block(
                CopyFilter::Client,
                BlockBody::Text {
                    text: "Только для клиента".to_owned(),
                    size_pt: Some(7.5),
                    bold: false,
                    align: Align::Center,
                },
            ),
            fields(vec![row("Неисправность", "{заказ.неисправность}", false)]),
        ],
    )
}

#[test]
fn two_per_a4_puts_both_copies_on_one_sheet() {
    let doc = rendered(
        &act_with_copies(),
        &data(&[
            ("order.number", "15"),
            ("order.malfunction", "Не включается"),
        ]),
    );
    let out = doc.text();
    assert_eq!(doc.page_count(), 1, "text was {out:?}");
    assert_a4(&doc);
    assert_eq!(out.matches("Экземпляр сервиса").count(), 1, "{out:?}");
    assert_eq!(out.matches("Экземпляр клиента").count(), 1, "{out:?}");
    assert!(out.find("Экземпляр сервиса") < out.find("Экземпляр клиента"));
    assert_eq!(out.matches("Только для сервиса").count(), 1, "{out:?}");
    assert_eq!(out.matches("Только для клиента").count(), 1, "{out:?}");
    assert_eq!(out.matches("Акт приёма № 15").count(), 2, "{out:?}");
    assert_eq!(out.matches("Не включается").count(), 2, "{out:?}");
}

#[test]
fn two_per_a4_prints_each_copy_on_its_own_sheet_when_a_copy_is_too_tall() {
    let lines: Vec<String> = (1..=40).map(|i| format!("Строка {i}")).collect();
    let doc = rendered(
        &act_with_copies(),
        &data(&[
            ("order.number", "15"),
            ("order.malfunction", &lines.join("\n")),
        ]),
    );
    let out = doc.text();
    assert_eq!(doc.page_count(), 2, "text was {out:?}");
    assert_a4(&doc);
    assert_eq!(out.matches("Экземпляр сервиса").count(), 1, "{out:?}");
    assert_eq!(out.matches("Экземпляр клиента").count(), 1, "{out:?}");
    assert!(out.find("Экземпляр сервиса") < out.find("Экземпляр клиента"));
    assert_eq!(out.matches("Строка 40").count(), 2, "{out:?}");
}

#[test]
fn one_per_page_prints_one_copy_without_copy_specific_blocks() {
    let mut t = act_with_copies();
    t.layout = Layout::OnePerPage;
    let out = rendered(&t, &data(&[("order.number", "15")])).text();
    assert_eq!(out.matches("Акт приёма № 15").count(), 1, "{out:?}");
    assert!(!out.contains("Экземпляр"), "{out:?}");
    assert!(!out.contains("Только для"), "{out:?}");
}

#[test]
fn row_with_hide_if_empty_is_not_printed_without_a_value() {
    let t = template(
        Layout::OnePerPage,
        vec![fields(vec![
            row("Цвет", "{заказ.цвет}", true),
            row("Предоплата", "{заказ.предоплата}", true),
            row("Мастер", "{заказ.мастер}", false),
        ])],
    );
    let out = rendered(&t, &data(&[("order.prepayment", "  ")])).text();
    assert!(!out.contains("Цвет"), "{out:?}");
    assert!(!out.contains("Предоплата"), "{out:?}");
    assert!(out.contains("Мастер"), "{out:?}");

    let out = rendered(&t, &data(&[("order.color", "Чёрный")])).text();
    assert!(out.contains("Цвет"), "{out:?}");
    assert!(out.contains("Чёрный"), "{out:?}");
}

#[test]
fn empty_value_prints_the_registry_text() {
    let t = template(
        Layout::OnePerPage,
        vec![
            fields(vec![
                row("Серийный номер", "{заказ.серийный_номер}", false),
                row("Адрес", "{клиент.адрес}", false),
                row("Мастер", "{заказ.мастер}", false),
            ]),
            text("Оценка: {заказ.оценка}"),
        ],
    );
    let out = rendered(&t, &data(&[])).text();
    assert_eq!(out.matches("Не указан").count(), 2, "{out:?}");
    assert!(out.contains("Оценка: Не оговорена"), "{out:?}");
    assert!(out.contains("Мастер"), "{out:?}");

    let out = rendered(&t, &data(&[("order.serial", "SN-1")])).text();
    assert_eq!(out.matches("Не указан").count(), 1, "{out:?}");
    assert!(out.contains("SN-1"), "{out:?}");
}

#[test]
fn blank_document_draws_lines_instead_of_empty_values() {
    let t = template(
        Layout::OnePerPage,
        vec![
            text("{фирма.название}"),
            fields(vec![
                row("Серийный номер", "{заказ.серийный_номер}", false),
                row("Цвет", "{заказ.цвет}", true),
            ]),
            text("Оценка: {заказ.оценка}"),
        ],
    );
    let mut d = data(&[("firm.name", "Сервис")]);
    d.blank = true;
    let doc = rendered(&t, &d);
    let out = doc.text();
    assert!(out.contains("Сервис"), "{out:?}");
    assert!(!out.contains("Не указан"), "{out:?}");
    assert!(!out.contains("Не оговорена"), "{out:?}");
    // A form to fill in by hand keeps every row.
    assert!(out.contains("Серийный номер"), "{out:?}");
    assert!(out.contains("Цвет"), "{out:?}");

    // The empty master is nothing in a document and a line in a form.
    let master = template(
        Layout::OnePerPage,
        vec![fields(vec![row("Мастер", "{заказ.мастер}", false)])],
    );
    let filled_in = rendered(&master, &data(&[])).png_pages(72).expect("png");
    let mut d = data(&[]);
    d.blank = true;
    let form = rendered(&master, &d).png_pages(72).expect("png");
    assert_ne!(filled_in, form);
}

#[test]
fn label_size_replaces_the_page_size() {
    let t = template(Layout::OnePerPage, vec![text("№ {заказ.номер}")]);
    let doc = render(
        &t,
        &data(&[("order.number", "5")]),
        &RenderOptions {
            label_mm: Some((40.0, 30.0)),
            offset_mm: (1.0, -1.5),
            dpi: Some(203),
        },
    )
    .expect("renders");
    let (w, h) = doc.page_size_mm();
    assert!(
        (w - 40.0).abs() < 0.01 && (h - 30.0).abs() < 0.01,
        "{w}x{h}"
    );
    assert_eq!(doc.page_count(), 1);
    assert!(doc.text().contains("№ 5"));
}

#[test]
fn landscape_and_custom_page_sizes() {
    let mut t = template(Layout::OnePerPage, vec![text("x")]);
    t.page.size = PageSize::A5;
    t.page.landscape = true;
    let (w, h) = rendered(&t, &data(&[])).page_size_mm();
    assert!(
        (w - 210.0).abs() < 0.01 && (h - 148.0).abs() < 0.01,
        "{w}x{h}"
    );
    t.page.size = PageSize::Custom {
        width_mm: 58.0,
        height_mm: 40.0,
    };
    t.page.landscape = false;
    let (w, h) = rendered(&t, &data(&[])).page_size_mm();
    assert!(
        (w - 58.0).abs() < 0.01 && (h - 40.0).abs() < 0.01,
        "{w}x{h}"
    );
}

#[test]
fn barcode_is_printed_only_for_a_valid_code() {
    let t = template(
        Layout::OnePerPage,
        vec![
            block(
                CopyFilter::Both,
                BlockBody::Columns {
                    columns: vec![
                        vec![text("Слева")],
                        vec![block(
                            CopyFilter::Both,
                            BlockBody::Barcode {
                                height_mm: 12.0,
                                digits: true,
                                align: Align::Right,
                            },
                        )],
                    ],
                    gap_mm: 4.0,
                },
            ),
            block(CopyFilter::Both, BlockBody::Separator),
            block(CopyFilter::Both, BlockBody::Spacer { mm: 3.0 }),
            text("Конец"),
        ],
    );
    let mut d = data(&[]);
    d.barcode = Some("200000012342".to_owned());
    let with_code = rendered(&t, &d);
    assert!(
        with_code.text().contains("200000012342"),
        "{:?}",
        with_code.text()
    );
    assert!(with_code.text().contains("Слева"));

    for code in [None, Some("200000012343"), Some("abc")] {
        d.barcode = code.map(str::to_owned);
        let doc = rendered(&t, &d);
        assert!(!doc.text().contains("2000000"), "{:?}", doc.text());
        assert!(doc.text().contains("Конец"));
    }
}

#[test]
fn template_survives_a_json_round_trip() {
    let t = act_with_copies();
    let json = serde_json::to_string(&t).expect("serializes");
    let back: Template = serde_json::from_str(&json).expect("deserializes");
    assert_eq!(back, t);
    assert!(json.contains(r#""layout":"two_per_a4""#), "{json}");
    assert!(json.contains(r#""type":"copy_label""#), "{json}");
}
