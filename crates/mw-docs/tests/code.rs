//! The author's own Typst code: what it prints and where its errors point.

use std::collections::BTreeMap;
use std::sync::Arc;

use mw_docs::model::{
    Align, Block, BlockBody, CodeDiagnostic, CodePlace, CopyFilter, Layout, Margins, PageSetup,
    PageSize, Template,
};
use mw_docs::{
    DocsError, DocumentData, RenderOptions, Rendered, render, standard_template, to_typst,
};
use mw_domain::documents::DocumentKind;

fn block(body: BlockBody) -> Block {
    Block {
        copies: CopyFilter::Both,
        body,
    }
}

fn text(text: &str) -> Block {
    block(BlockBody::Text {
        text: text.to_owned(),
        size_pt: None,
        bold: false,
        align: Align::Left,
        one_line: false,
    })
}

fn code(code: &str) -> Block {
    block(BlockBody::TypstCode {
        code: code.to_owned(),
    })
}

fn columns(columns: Vec<Vec<Block>>) -> Block {
    block(BlockBody::Columns {
        columns,
        gap_mm: 4.0,
    })
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
        logo: None,
        blank: false,
    }
}

fn rendered(template: &Template, data: &DocumentData) -> Rendered {
    render(template, data, &RenderOptions::default()).unwrap_or_else(|e| panic!("renders: {e}"))
}

/// The diagnostics of a template that must not compile.
fn diagnostics(template: &Template) -> Vec<CodeDiagnostic> {
    match render(template, &data(&[]), &RenderOptions::default()) {
        Err(DocsError::Code(diagnostics)) => diagnostics,
        Err(other) => panic!("expected a code error, got {other}"),
        Ok(_) => panic!("expected a code error, the template compiled"),
    }
}

fn place(block: Vec<usize>, line: u32, column: u32) -> Option<CodePlace> {
    Some(CodePlace {
        block,
        line,
        column,
    })
}

/// A grey PNG of `width` × `height` pixels.
fn png(width: u32, height: u32) -> Arc<[u8]> {
    let image = image::RgbImage::from_pixel(width, height, image::Rgb([40, 90, 160]));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap_or_else(|e| panic!("png: {e}"));
    Arc::from(bytes.into_inner())
}

#[test]
fn code_block_prints_its_content() {
    let t = template(
        Layout::OnePerPage,
        vec![code(r#"КОД-ПРОВЕРКА #v("order.number")"#)],
    );
    let out = rendered(&t, &data(&[("order.number", "A-17")])).text();
    assert!(out.contains("КОД-ПРОВЕРКА A-17"), "text was {out:?}");
}

#[test]
fn code_knows_its_copy() {
    let two = rendered(&template(Layout::TwoPerA4, vec![code("#copy")]), &data(&[])).text();
    assert!(
        two.contains("service") && two.contains("client"),
        "text was {two:?}"
    );
    let one = rendered(
        &template(Layout::OnePerPage, vec![code("#copy")]),
        &data(&[]),
    )
    .text();
    assert!(one.contains("single"), "text was {one:?}");
}

#[test]
fn code_in_a_column_prints() {
    let t = template(
        Layout::OnePerPage,
        vec![columns(vec![vec![code("ЛЕВО")], vec![text("ПРАВО")]])],
    );
    let out = rendered(&t, &data(&[])).text();
    assert!(
        out.contains("ЛЕВО") && out.contains("ПРАВО"),
        "text was {out:?}"
    );
}

#[test]
fn empty_code_prints_nothing() {
    let doc = rendered(&template(Layout::OnePerPage, vec![code("")]), &data(&[]));
    assert_eq!(doc.page_count(), 1);
    let png = |blocks| {
        rendered(&template(Layout::OnePerPage, blocks), &data(&[]))
            .png_pages(72)
            .expect("png")
    };
    assert_eq!(
        png(vec![text("до"), code(""), text("после")]),
        png(vec![text("до"), text("после")])
    );
}

#[test]
fn syntax_error_points_at_the_block_line() {
    let d = diagnostics(&template(
        Layout::OnePerPage,
        vec![text("до"), code("строка\n#let x = (\n")],
    ));
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].place, place(vec![1], 2, 10), "{d:?}");
}

#[test]
fn unknown_variable_has_line_and_column() {
    let d = diagnostics(&template(Layout::OnePerPage, vec![code("abc #nope def")]));
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].place, place(vec![0], 1, 6), "{d:?}");
    assert!(d[0].message.contains("unknown variable: nope"), "{d:?}");
}

#[test]
fn error_inside_a_helper_points_at_the_call() {
    let d = diagnostics(&template(Layout::OnePerPage, vec![code("\n#v(5)")]));
    assert_eq!(d.len(), 1, "{d:?}");
    let place = d[0].place.as_ref().expect("a place in the code");
    assert_eq!((place.block.as_slice(), place.line), (&[0][..], 2), "{d:?}");
}

#[test]
fn two_copies_report_one_error() {
    let d = diagnostics(&template(Layout::TwoPerA4, vec![code("#nope")]));
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].place, place(vec![0], 1, 2), "{d:?}");
}

#[test]
fn error_in_a_column_names_the_full_path() {
    let d = diagnostics(&template(
        Layout::OnePerPage,
        vec![
            text("до"),
            columns(vec![vec![text("слева")], vec![code("#nope")]]),
        ],
    ));
    assert_eq!(d.len(), 1, "{d:?}");
    let place = d[0].place.as_ref().expect("a place in the code");
    assert_eq!(place.block, vec![1, 1, 0], "{d:?}");
}

#[test]
fn code_is_sandboxed() {
    for source in [
        r#"#read("/etc/passwd")"#,
        r#"#import "@preview/cetz:0.3.0": *"#,
        "#datetime.today()",
    ] {
        let d = diagnostics(&template(Layout::OnePerPage, vec![code(source)]));
        assert!(!d.is_empty(), "{source}");
        for diagnostic in &d {
            let place = diagnostic.place.as_ref().expect("a place in the code");
            assert_eq!((place.block.as_slice(), place.line), (&[0][..], 1), "{d:?}");
            let said = format!("{} {}", diagnostic.message, diagnostic.hints.join(" "));
            assert!(!said.contains("root:"), "{source}: {said}");
        }
    }
}

#[test]
fn code_can_show_the_logo() {
    let t = template(
        Layout::OnePerPage,
        vec![code(r#"#if data.logo { image("logo.png", width: 1cm) }"#)],
    );
    let has_image = |logo: Option<Arc<[u8]>>| {
        let d = DocumentData { logo, ..data(&[]) };
        let pdf = rendered(&t, &d).pdf().expect("pdf");
        let image = b"/Subtype/Image";
        pdf.windows(image.len()).any(|w| w == image)
    };
    assert!(has_image(Some(png(40, 20))), "no image in the PDF");
    assert!(!has_image(None), "an image without a logo");
}

#[test]
fn template_without_code_is_unchanged() {
    for kind in DocumentKind::ALL {
        let source = to_typst(&standard_template(kind), &RenderOptions::default());
        assert!(!source.contains("/mw.typ"), "{kind:?}");
        assert!(!source.contains("include"), "{kind:?}");
    }
}
