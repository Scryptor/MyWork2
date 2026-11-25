use mw_docs::{DocsError, compile_for_tests, font_covers};
use mw_domain::pages::PageSelection;

const TEXT: &str = "Привет, №1 «тест» — ёлка";

#[test]
fn cyrillic_document_compiles_to_pdf_with_text() {
    let doc = compile_for_tests(TEXT.to_owned(), vec![]).expect("compiles");
    assert!(doc.pdf().expect("pdf").starts_with(b"%PDF-"));
    assert!(doc.text().contains(TEXT), "text was {:?}", doc.text());
    assert_eq!(doc.page_count(), 1);
}

#[test]
fn label_page_has_exact_size_and_png_pixels() {
    let doc = compile_for_tests(
        "#set page(width: 58mm, height: 40mm, margin: 0pt)\nx".to_owned(),
        vec![],
    )
    .expect("compiles");
    let (w, h) = doc.page_size_mm();
    assert!(
        (w - 58.0).abs() < 0.01 && (h - 40.0).abs() < 0.01,
        "{w}x{h}"
    );
    let pngs = doc.png_pages(203).expect("png");
    assert_eq!(pngs.len(), 1);
    assert!(pngs[0].starts_with(&[0x89, b'P', b'N', b'G']));
    let (pw, ph) = png_size(&pngs[0]);
    assert!(
        (pw as i64 - 463).abs() <= 1 && (ph as i64 - 319).abs() <= 1,
        "{pw}x{ph}"
    );
}

#[test]
fn markup_error_is_reported_without_panic() {
    match compile_for_tests("#nonexistent-function()".to_owned(), vec![]) {
        Err(DocsError::Compile(errors)) => {
            assert!(!errors.is_empty());
            assert!(
                errors.iter().any(|e| e.contains("nonexistent")),
                "{errors:?}"
            );
        }
        other => panic!("expected compile error, got {:?}", other.map(|_| ())),
    }
}

#[test]
fn virtual_files_are_readable() {
    let doc = compile_for_tests(
        "#let d = json(\"data.json\")\n#d.name".to_owned(),
        vec![(
            "data.json".to_owned(),
            r#"{"name":"Ёж"}"#.as_bytes().to_vec(),
        )],
    )
    .expect("compiles");
    assert!(doc.text().contains("Ёж"));
}

#[test]
fn font_coverage() {
    assert!(
        font_covers("АБВГДЕЖЗИЙКЛМНОПРСТУФХЦЧШЩЪЫЬЭЮЯабвгдежзийклмнопрстуфхцчшщъыьэюяЁё№«»—–…·×°")
            .is_empty()
    );
    assert_eq!(font_covers("₽"), vec!['₽']);
}

#[test]
fn many_renders_in_a_row_work() {
    for i in 0..100 {
        let doc = compile_for_tests(format!("Документ {i}"), vec![]).expect("compiles");
        assert_eq!(doc.page_count(), 1);
    }
}

fn png_size(png: &[u8]) -> (u32, u32) {
    let w = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
    let h = u32::from_be_bytes([png[20], png[21], png[22], png[23]]);
    (w, h)
}

#[test]
fn invalid_virtual_file_name_is_an_error() {
    let r = compile_for_tests("x".to_owned(), vec![("../../etc/x".to_owned(), vec![])]);
    assert!(
        matches!(r, Err(DocsError::BadFileName(_))),
        "{:?}",
        r.map(|_| ())
    );
}

/// Страниц в PDF: объектов `/Type /Page`, за которыми не идёт буква — `/Pages` (дерево страниц) и
/// `/PageLabel` (номер выбранной страницы) не в счёт; typst-pdf пишет их без пробела, `/Type/Page`.
fn pdf_page_count(pdf: &[u8]) -> usize {
    let text = String::from_utf8_lossy(pdf);
    text.match_indices("/Type")
        .filter(|(at, mark)| {
            let rest = text[at + mark.len()..].trim_start();
            rest.strip_prefix("/Page")
                .is_some_and(|after| !after.starts_with(|c: char| c.is_ascii_alphabetic()))
        })
        .count()
}

#[test]
fn only_the_chosen_pages_are_exported() {
    let doc =
        compile_for_tests("Первая\n#pagebreak()\nВторая".to_owned(), vec![]).expect("compiles");
    assert_eq!(doc.page_count(), 2);
    let second = PageSelection::parse("2").expect("selection");

    let all = doc.png_pages(72).expect("png");
    assert_eq!(all.len(), 2);
    let chosen = doc.png_selected(72, Some(&second)).expect("png");
    assert_eq!(chosen, [all[1].clone()], "только вторая страница");
    assert_eq!(doc.png_selected(72, None).expect("png"), all);

    assert_eq!(pdf_page_count(&doc.pdf().expect("pdf")), 2);
    let pdf = doc.pdf_selected(Some(&second)).expect("pdf");
    assert!(pdf.starts_with(b"%PDF-"));
    assert_eq!(pdf_page_count(&pdf), 1);

    // Страница за концом документа, в том числе среди существующих, — отказ, а не молчаливый пропуск.
    for beyond in ["5", "2-3"] {
        let beyond = PageSelection::parse(beyond).expect("selection");
        assert_eq!(doc.png_selected(72, Some(&beyond)), Err(DocsError::NoPages));
        assert_eq!(doc.pdf_selected(Some(&beyond)), Err(DocsError::NoPages));
    }
}
