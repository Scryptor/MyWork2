use mw_docs::{Rendered, compile_for_tests, merge};

fn label(text: &str) -> Rendered {
    compile_for_tests(
        format!("#set page(width: 58mm, height: 40mm, margin: 0pt)\n{text}"),
        vec![],
    )
    .expect("compiles")
}

#[test]
fn merged_pdf_has_all_pages() {
    let merged = merge(vec![label("один"), label("два"), label("три")]);
    assert_eq!(merged.page_count(), 3);
    assert!(merged.pdf().expect("pdf").starts_with(b"%PDF"));
    assert_eq!(merged.png_pages(203).expect("png").len(), 3);
    let text = merged.text();
    assert!(
        text.find("один") < text.find("два") && text.find("два") < text.find("три"),
        "порядок страниц: {text:?}"
    );
}

#[test]
fn merging_nothing_gives_an_empty_document() {
    let merged = merge(Vec::new());
    assert_eq!(merged.page_count(), 0);
}
