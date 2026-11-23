use mw_docs::model::{BlockBody, Layout};
use mw_docs::{
    DocumentData, RenderOptions, Rendered, font_covers, render, sample_data, standard_template,
    to_typst,
};
use mw_domain::documents::DocumentKind;

const CLIENT: &str = "Петров Пётр Петрович";
const BARCODE: &str = "200000012342";

/// The acts and the title each prints with the sample data.
const ACTS: [(DocumentKind, &str); 4] = [
    (
        DocumentKind::IntakeAct,
        "Акт приёма № 1234 от 22 ноября 2026",
    ),
    (
        DocumentKind::IssueAct,
        "Акт выдачи № 1234 от 25 ноября 2026",
    ),
    (
        DocumentKind::WarrantyIntakeAct,
        "Акт приёма по гарантии № 1234 от 22 ноября 2026",
    ),
    (
        DocumentKind::WarrantyIssueAct,
        "Акт выдачи после гарантийного ремонта № 1234 от 25 ноября 2026",
    ),
];

fn rendered(kind: DocumentKind, data: &DocumentData, opts: &RenderOptions) -> Rendered {
    render(&standard_template(kind), data, opts).unwrap_or_else(|e| panic!("{kind:?}: {e}"))
}

fn sample(kind: DocumentKind) -> Rendered {
    rendered(kind, &sample_data(kind), &RenderOptions::default())
}

fn label(width: f32, height: f32) -> RenderOptions {
    RenderOptions {
        label_mm: Some((width, height)),
        offset_mm: (0.0, 0.0),
        dpi: Some(203),
    }
}

fn assert_page(doc: &Rendered, width: f64, height: f64) {
    assert_eq!(doc.page_count(), 1, "{:?}", doc.text());
    let (w, h) = doc.page_size_mm();
    assert!(
        (w - width).abs() < 0.01 && (h - height).abs() < 0.01,
        "{w}x{h}"
    );
}

/// The text has no substitution left and every glyph is in the embedded font.
fn assert_printable(kind: DocumentKind, out: &str) {
    assert!(!out.contains('{'), "{kind:?}: {out:?}");
    assert!(
        font_covers(out).is_empty(),
        "{kind:?}: {:?}",
        font_covers(out)
    );
}

#[test]
fn every_kind_has_its_own_template() {
    for kind in DocumentKind::ALL {
        assert_eq!(standard_template(kind).kind, kind);
    }
}

#[test]
fn acts_print_both_copies_on_one_a4_sheet() {
    for (kind, title) in ACTS {
        let template = standard_template(kind);
        assert_eq!(template.layout, Layout::TwoPerA4, "{kind:?}");
        assert_eq!(template.font_size_pt, 9.0, "{kind:?}");
        let doc = sample(kind);
        assert_page(&doc, 210.0, 297.0);
        let out = doc.text();
        assert_printable(kind, &out);
        assert_eq!(out.matches(title).count(), 2, "{kind:?}: {out:?}");
        assert_eq!(out.matches(CLIENT).count(), 2, "{kind:?}: {out:?}");
        assert_eq!(out.matches(BARCODE).count(), 2, "{kind:?}: {out:?}");
        assert_eq!(out.matches("Сервисный центр «Пример»").count(), 2);
        assert_eq!(out.matches("Экземпляр сервиса").count(), 1, "{out:?}");
        assert_eq!(out.matches("Экземпляр клиента").count(), 1, "{out:?}");
        assert_eq!(out.matches("ИНН 000000000000").count(), 2, "{out:?}");
    }
}

#[test]
fn acts_have_their_own_rows_terms_and_signatures() {
    let intake = sample(DocumentKind::IntakeAct).text();
    for row in [
        "Адрес",
        "Цвет",
        "Комплектность",
        "Внешний вид",
        "Предварительная стоимостьот 1 000 до 2 500 руб.",
        "Предоплата500 руб.",
        "Срок диагностики",
        "Сдал",
        "Принял",
    ] {
        assert_eq!(intake.matches(row).count(), 2, "{row}: {intake:?}");
    }
    assert!(!intake.contains("Гарантия"), "{intake:?}");
    assert!(!intake.contains("К оплате"), "{intake:?}");

    let issue = sample(DocumentKind::IssueAct).text();
    for row in [
        "Выполненные работыЗамена разъёма питания",
        "МастерИванов И. И.",
        "Гарантия30 дней",
        "Стоимость2 000 руб.",
        "Скидка200 руб.",
        "Предоплата500 руб.",
        "К оплате1 300 руб.",
        "претензий к внешнему виду",
        "Получил",
        "Выдал",
    ] {
        assert_eq!(issue.matches(row).count(), 2, "{row}: {issue:?}");
    }
    assert!(!issue.contains("Комплектность"), "{issue:?}");

    for kind in [
        DocumentKind::WarrantyIntakeAct,
        DocumentKind::WarrantyIssueAct,
    ] {
        let out = sample(kind).text();
        assert_eq!(
            out.matches("Гарантия30 дней").count(),
            2,
            "{kind:?}: {out:?}"
        );
    }
}

#[test]
fn empty_fields_print_mywork2_texts_or_hide_the_row() {
    let mut data = sample_data(DocumentKind::IntakeAct);
    for key in [
        "client.address",
        "client.phone",
        "order.serial",
        "order.estimate",
        "order.color",
        "order.prepayment",
    ] {
        data.values.remove(key);
    }
    let out = rendered(DocumentKind::IntakeAct, &data, &RenderOptions::default()).text();
    assert_printable(DocumentKind::IntakeAct, &out);
    assert_eq!(out.matches("АдресНе указан").count(), 2, "{out:?}");
    assert_eq!(out.matches("Серийный номерНе указан").count(), 2, "{out:?}");
    assert_eq!(
        out.matches("Предварительная стоимостьНе оговорена").count(),
        2,
        "{out:?}"
    );
    // An order without phones has no phone row.
    assert!(!out.contains("Телефон"), "{out:?}");
    assert!(!out.contains("Цвет"), "{out:?}");
    assert!(!out.contains("Предоплата"), "{out:?}");

    let mut data = sample_data(DocumentKind::IssueAct);
    for key in [
        "order.master",
        "order.discount",
        "order.prepayment",
        "order.issued",
        "client.phone",
    ] {
        data.values.remove(key);
    }
    let out = rendered(DocumentKind::IssueAct, &data, &RenderOptions::default()).text();
    assert_printable(DocumentKind::IssueAct, &out);
    assert_eq!(out.matches("Акт выдачи № 1234 от").count(), 2, "{out:?}");
    // The sample firm is on «ул. Мастеров».
    let out = out.replace("Мастеров", "");
    for hidden in ["Мастер", "Скидка", "Предоплата", "Телефон"] {
        assert!(!out.contains(hidden), "{hidden}: {out:?}");
    }
    assert_eq!(out.matches("К оплате1 300 руб.").count(), 2, "{out:?}");
}

#[test]
fn blank_intake_act_is_a_form_without_order_number_and_barcode() {
    let data = sample_data(DocumentKind::BlankIntakeAct);
    assert!(data.blank);
    assert_eq!(data.barcode, None);
    let doc = sample(DocumentKind::BlankIntakeAct);
    assert_page(&doc, 210.0, 297.0);
    let out = doc.text();
    assert_printable(DocumentKind::BlankIntakeAct, &out);
    assert_eq!(out.matches("Акт приёма №").count(), 2, "{out:?}");
    assert!(!out.contains("1234"), "{out:?}");
    assert!(!out.contains("2000000"), "{out:?}");
    assert!(!out.contains(CLIENT), "{out:?}");
    assert!(!out.contains("Не указан"), "{out:?}");
    assert!(!out.contains("Не оговорена"), "{out:?}");
    // The firm's own details are printed; a form keeps every row to fill in.
    assert_eq!(out.matches("Сервисный центр «Пример»").count(), 2);
    assert_eq!(out.matches("Цвет").count(), 2, "{out:?}");
    assert_eq!(out.matches("Предоплата").count(), 2, "{out:?}");
    assert_eq!(out.matches("Телефон").count(), 2, "{out:?}");
}

#[test]
fn sticker_fits_58x40_and_40x30_labels() {
    let template = standard_template(DocumentKind::Sticker);
    assert_eq!(template.layout, Layout::OnePerPage);
    let data = sample_data(DocumentKind::Sticker);
    for (opts, width, height) in [
        (RenderOptions::default(), 58.0, 40.0),
        (label(58.0, 40.0), 58.0, 40.0),
        (label(40.0, 30.0), 40.0, 30.0),
    ] {
        let doc = rendered(DocumentKind::Sticker, &data, &opts);
        assert_page(&doc, width, height);
        let out = doc.text();
        assert_printable(DocumentKind::Sticker, &out);
        for line in [
            "№ 1234 · 22 ноября 2026",
            CLIENT,
            "+7 900 000-00-00",
            BARCODE,
        ] {
            assert_eq!(out.matches(line).count(), 1, "{line}: {out:?}");
        }
        // The device line is in one row: whole on the wide label, cut with an ellipsis on the narrow one.
        if width > 50.0 {
            assert_eq!(
                out.matches("Смартфон Samsung Galaxy A52").count(),
                1,
                "{out:?}"
            );
        } else {
            assert!(out.contains("Смартфон Samsung"), "{out:?}");
            assert!(out.trim_end().ends_with('…'), "{out:?}");
            assert!(!out.contains("A52"), "{out:?}");
        }
    }
}

/// Width in mm of the barcode image in the Typst source.
fn barcode_width_mm(source: &str) -> Option<f64> {
    let key = "image(\"barcode.svg\", width: ";
    let start = source.find(key)? + key.len();
    let end = start + source[start..].find("mm")?;
    source[start..end].parse().ok()
}

#[test]
fn sticker_barcode_fits_the_label_width() {
    let template = standard_template(DocumentKind::Sticker);
    let margins = template.page.margins_mm.left + template.page.margins_mm.right;
    let dot = 25.4 / 203.0;
    // 40×30 at 203 dpi: the usual 3 dots (42.4 mm) do not fit 37 mm, so the module is 2 dots.
    let narrow = barcode_width_mm(&to_typst(&template, &label(40.0, 30.0))).expect("barcode");
    assert!(narrow <= f64::from(40.0 - margins), "{narrow}");
    assert!((narrow - 113.0 * 2.0 * dot).abs() < 0.01, "{narrow}");
    // 58×40 keeps 3 dots.
    let wide = barcode_width_mm(&to_typst(&template, &label(58.0, 40.0))).expect("barcode");
    assert!((wide - 113.0 * 3.0 * dot).abs() < 0.01, "{wide}");
    assert!(wide <= f64::from(58.0 - margins), "{wide}");
}

#[test]
fn calibration_label_shows_size_dpi_and_offset() {
    let template = standard_template(DocumentKind::Calibration);
    assert_eq!(template.layout, Layout::OnePerPage);
    assert!(
        template
            .blocks
            .iter()
            .any(|b| b.body == BlockBody::CalibrationMarks)
    );
    assert_eq!(
        sample_data(DocumentKind::Calibration),
        DocumentData::default()
    );

    let doc = sample(DocumentKind::Calibration);
    assert_page(&doc, 58.0, 40.0);
    let out = doc.text();
    assert_printable(DocumentKind::Calibration, &out);
    assert!(out.contains("58 × 40 мм"), "{out:?}");
    assert!(out.contains("смещение 0/0"), "{out:?}");
    assert!(!out.contains("dpi"), "{out:?}");

    let opts = RenderOptions {
        label_mm: Some((40.0, 30.0)),
        offset_mm: (1.0, -0.5),
        dpi: Some(203),
    };
    let doc = rendered(
        DocumentKind::Calibration,
        &sample_data(DocumentKind::Calibration),
        &opts,
    );
    assert_page(&doc, 40.0, 30.0);
    let out = doc.text();
    assert_printable(DocumentKind::Calibration, &out);
    assert!(out.contains("40 × 30 мм · 203 dpi"), "{out:?}");
    assert!(out.contains("смещение 1/−0,5"), "{out:?}");
}

#[test]
fn sample_data_comes_from_the_field_samples() {
    let data = sample_data(DocumentKind::IntakeAct);
    assert!(!data.blank);
    assert_eq!(
        data.barcode,
        mw_domain::barcode::order_barcode(1234),
        "the sample order is number 1234"
    );
    for field in mw_docs::fields::fields() {
        assert_eq!(
            data.values.get(field.key).map(String::as_str),
            Some(field.sample),
            "{}",
            field.key
        );
    }
    let blank = sample_data(DocumentKind::BlankIntakeAct);
    assert!(blank.values.contains_key("firm.name"));
    assert!(
        blank
            .values
            .keys()
            .all(|k| !k.starts_with("order.") && !k.starts_with("client.")),
        "{:?}",
        blank.values
    );
}

/// Not a test: PNG of every standard template on its sample data at 110 dpi in
/// `$CARGO_TARGET_DIR/docs-preview/`, to look at. `cargo test -p mw-docs --test standard -- --ignored`.
#[test]
#[ignore = "writes preview images to look at"]
fn write_previews() {
    let target = std::env::var("CARGO_TARGET_DIR")
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../../target").to_owned());
    let dir = std::path::Path::new(&target).join("docs-preview");
    std::fs::create_dir_all(&dir).expect("preview folder");
    let mut previews: Vec<(String, Rendered)> = DocumentKind::ALL
        .into_iter()
        .map(|kind| (kind.as_str().to_owned(), sample(kind)))
        .collect();
    // Terms as long as a workshop's real contract: still two copies on one sheet.
    let mut long_terms = sample_data(DocumentKind::IntakeAct);
    long_terms.values.insert(
        "firm.intake_terms".to_owned(),
        [
            "1. Устройство принято на диагностику без вскрытия и проверки внутренних повреждений.",
            "2. Срок диагностики — до 3 рабочих дней, срок ремонта — до 45 дней со дня приёма.",
            "3. Сервис не отвечает за данные в памяти устройства; сохраните их заранее.",
            "4. Скрытые неисправности, найденные при диагностике, могут изменить стоимость ремонта; \
             работы сверх оценки выполняются только с согласия клиента.",
            "5. Устройство, не полученное в течение 3 месяцев после уведомления о готовности, \
             хранится платно.",
            "6. Устройство выдаётся по предъявлении этого акта или документа, удостоверяющего личность.",
        ]
        .join("\n"),
    );
    let doc = rendered(
        DocumentKind::IntakeAct,
        &long_terms,
        &RenderOptions::default(),
    );
    previews.push(("intake_act_long_terms".to_owned(), doc));
    for kind in [DocumentKind::Sticker, DocumentKind::Calibration] {
        let opts = RenderOptions {
            offset_mm: (0.0, 0.0),
            ..label(40.0, 30.0)
        };
        let name = format!("{}_40x30_203dpi", kind.as_str());
        previews.push((name, rendered(kind, &sample_data(kind), &opts)));
        let name = format!("{}_58x40_203dpi", kind.as_str());
        previews.push((name, rendered(kind, &sample_data(kind), &label(58.0, 40.0))));
    }
    let shifted = RenderOptions {
        offset_mm: (2.0, -1.5),
        ..label(40.0, 30.0)
    };
    let doc = rendered(
        DocumentKind::Calibration,
        &sample_data(DocumentKind::Calibration),
        &shifted,
    );
    previews.push(("calibration_40x30_203dpi_shifted".to_owned(), doc));
    for (name, doc) in previews {
        for (i, png) in doc.png_pages(110).expect("png").iter().enumerate() {
            std::fs::write(dir.join(format!("{name}-{}.png", i + 1)), png).expect("write");
        }
        // Labels are a few hundred pixels at 110 dpi: also a close-up.
        if name.contains("_203dpi") {
            let png = &doc.png_pages(400).expect("png")[0];
            std::fs::write(dir.join(format!("{name}-closeup.png")), png).expect("write");
        }
    }
}
