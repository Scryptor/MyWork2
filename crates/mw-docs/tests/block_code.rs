//! A block turned into the author's own Typst code prints what the block printed.

use std::sync::Arc;

use mw_docs::model::{
    Align, Block, BlockBody, CopyFilter, Layout, Margins, PageSetup, PageSize, Template,
};
use mw_docs::{
    DocumentData, RenderOptions, Rendered, block_code, render, sample_data, standard_template,
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

/// A logo of 40 × 15 pixels.
fn logo() -> Arc<[u8]> {
    let image = image::RgbImage::from_pixel(40, 15, image::Rgb([40, 90, 160]));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap_or_else(|e| panic!("png: {e}"));
    Arc::from(bytes.into_inner())
}

fn rendered(template: &Template, data: &DocumentData) -> Rendered {
    render(template, data, &RenderOptions::default()).unwrap_or_else(|e| panic!("renders: {e}"))
}

fn pages(template: &Template, data: &DocumentData) -> Vec<Vec<u8>> {
    rendered(template, data)
        .png_pages(72)
        .unwrap_or_else(|e| panic!("png: {e}"))
}

/// The paths of the blocks: the top level and the blocks in columns.
fn paths(blocks: &[Block]) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    for (i, block) in blocks.iter().enumerate() {
        out.push(vec![i]);
        if let BlockBody::Columns { columns, .. } = &block.body {
            for (c, column) in columns.iter().enumerate() {
                out.extend((0..column.len()).map(|j| vec![i, c, j]));
            }
        }
    }
    out
}

fn block_at<'a>(blocks: &'a mut [Block], path: &[usize]) -> &'a mut Block {
    match path {
        [i] => &mut blocks[*i],
        [i, c, rest @ ..] => match &mut blocks[*i].body {
            BlockBody::Columns { columns, .. } => block_at(&mut columns[*c], rest),
            _ => panic!("no columns at {path:?}"),
        },
        _ => panic!("bad path {path:?}"),
    }
}

/// `template` with the block at `path` turned into `code`; its `copies` stay.
fn converted(template: &Template, path: &[usize], code: String) -> Template {
    let mut out = template.clone();
    block_at(&mut out.blocks, path).body = BlockBody::TypstCode { code };
    out
}

fn has_barcode(body: &BlockBody) -> bool {
    match body {
        BlockBody::Barcode { .. } => true,
        BlockBody::Columns { columns, .. } => {
            columns.iter().flatten().any(|b| has_barcode(&b.body))
        }
        _ => false,
    }
}

#[test]
fn converted_blocks_print_the_same() {
    for kind in DocumentKind::ALL.into_iter().filter(|k| k.editable()) {
        let template = standard_template(kind);
        let data = DocumentData {
            logo: Some(logo()),
            ..sample_data(kind)
        };
        let expected = pages(&template, &data);
        let mut count = 0;
        for path in paths(&template.blocks) {
            let Some(code) = block_code(&template, &path) else {
                // Only the barcode and the columns that hold it stay blocks.
                let mut scratch = template.clone();
                let body = &block_at(&mut scratch.blocks, &path).body;
                assert!(has_barcode(body), "{kind:?} {path:?} does not convert");
                continue;
            };
            let got = pages(&converted(&template, &path, code.clone()), &data);
            assert!(
                got == expected,
                "{kind:?} {path:?} prints otherwise:\n{code}"
            );
            count += 1;
        }
        assert!(count > 0, "{kind:?}: nothing converted");
    }
}

#[test]
fn columns_with_code_convert() {
    let columns = block(BlockBody::Columns {
        columns: vec![
            vec![
                code("ЛЕВО #v(\"order.number\") // до конца строки"),
                text("слева"),
            ],
            vec![
                code("#block(width: 100%, align(right, text(copy)))"),
                block(BlockBody::CopyLabel { align: Align::Left }),
            ],
        ],
        gap_mm: 4.0,
    });
    let data = DocumentData {
        values: [("order.number".to_owned(), "A-17".to_owned())].into(),
        ..DocumentData::default()
    };
    for layout in [Layout::OnePerPage, Layout::TwoPerA4] {
        let t = template(layout, vec![text("до"), columns.clone(), text("после")]);
        let code = block_code(&t, &[1]).expect("columns with code convert");
        assert!(code.contains("// до конца строки"), "{code}");
        let got = pages(&converted(&t, &[1], code.clone()), &data);
        assert!(
            got == pages(&t, &data),
            "{layout:?} prints otherwise:\n{code}"
        );
    }
}

#[test]
fn copy_label_converts_with_copy() {
    let label = template(
        Layout::TwoPerA4,
        vec![block(BlockBody::CopyLabel {
            align: Align::Right,
        })],
    );
    let own = block_code(&label, &[0]).expect("a copy label converts");
    assert!(own.contains(r#"copy == "service""#), "{own}");
    let two = rendered(
        &converted(&label, &[0], own.clone()),
        &DocumentData::default(),
    )
    .text();
    assert!(
        two.contains("Экземпляр сервиса") && two.contains("Экземпляр клиента"),
        "text was {two:?}"
    );
    // The only copy has no label.
    let one = template(Layout::OnePerPage, vec![code(&own)]);
    let one = rendered(&one, &DocumentData::default()).text();
    assert!(!one.contains("Экземпляр"), "text was {one:?}");
}

#[test]
fn code_has_one_branch_when_copies_print_alike() {
    let t = template(Layout::TwoPerA4, vec![text("Привет")]);
    let code = block_code(&t, &[0]).expect("a text converts");
    assert!(code.starts_with("#{\n"), "{code}");
    assert!(code.ends_with("\n}\n"), "{code}");
    assert!(!code.contains("copy =="), "{code}");
    let label = template(
        Layout::TwoPerA4,
        vec![block(BlockBody::CopyLabel {
            align: Align::Right,
        })],
    );
    let code = block_code(&label, &[0]).expect("a copy label converts");
    assert!(
        code.starts_with("#if copy == \"service\" {\n")
            && code.contains("\n} else if copy == \"client\" {\n")
            && code.contains("\n} else {\n")
            && code.ends_with("\n}\n"),
        "{code}"
    );
}

#[test]
fn what_does_not_convert() {
    let sticker = standard_template(DocumentKind::Sticker);
    assert!(
        matches!(sticker.blocks[0].body, BlockBody::Barcode { .. }),
        "the sticker starts with its barcode"
    );
    assert_eq!(block_code(&sticker, &[0]), None, "barcode");
    let act = standard_template(DocumentKind::IntakeAct);
    assert!(
        matches!(act.blocks[0].body, BlockBody::Columns { .. }),
        "the act starts with columns"
    );
    assert_eq!(block_code(&act, &[0]), None, "columns with a barcode");
    assert_eq!(block_code(&act, &[0, 1, 0]), None, "barcode in a column");
    assert_eq!(block_code(&act, &[0, 5, 0]), None, "no such column");
    assert_eq!(block_code(&act, &[0, 0]), None, "a column is not a block");
    let calibration = standard_template(DocumentKind::Calibration);
    assert_eq!(block_code(&calibration, &[0]), None, "calibration marks");
    let own = template(Layout::OnePerPage, vec![text("текст"), code("#copy")]);
    assert_eq!(block_code(&own, &[1]), None, "code");
    assert_eq!(block_code(&own, &[99]), None, "no such block");
    assert_eq!(block_code(&own, &[0, 5, 0]), None, "not columns");
    assert_eq!(block_code(&own, &[]), None, "empty path");
    assert!(block_code(&own, &[0]).is_some(), "a text converts");
    // In a file of its own an open bracket is text; in the code of the columns it would take their
    // closing one.
    let open = template(
        Layout::OnePerPage,
        vec![block(BlockBody::Columns {
            columns: vec![vec![code("a [ b")], vec![text("x")]],
            gap_mm: 4.0,
        })],
    );
    let printed = rendered(&open, &DocumentData::default()).text();
    assert!(printed.contains("a [ b"), "text was {printed:?}");
    assert_eq!(
        block_code(&open, &[0]),
        None,
        "columns with an open bracket"
    );
}

#[test]
fn converted_code_keeps_data_out() {
    let number = sample_data(DocumentKind::IntakeAct)
        .values
        .get("order.number")
        .cloned()
        .expect("a sample order number");
    assert!(!number.trim().is_empty());
    let t = template(Layout::OnePerPage, vec![text("Заказ {заказ.номер}")]);
    let code = block_code(&t, &[0]).expect("a text converts");
    assert!(code.contains(r#"v("order.number")"#), "{code}");
    assert!(!code.contains(&number), "{code}");
}
