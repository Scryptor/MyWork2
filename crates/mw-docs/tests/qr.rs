//! The QR block in documents: the printed code reads back as its link, and the link never enters the
//! markup.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mw_docs::model::{
    Align, Block, BlockBody, CopyFilter, Layout, Margins, PageSetup, PageSize, Template,
};
use mw_docs::{DocumentData, RenderOptions, render, sample_data, standard_template, to_typst};
use mw_domain::documents::DocumentKind;

const TELEGRAM: &str = "https://t.me/example_service";
const YANDEX: &str = "https://yandex.ru/maps/?text=example";

fn qr(link: &str, size_mm: f32, caption: &str) -> Block {
    Block {
        copies: CopyFilter::Both,
        body: BlockBody::Qr {
            link: link.to_owned(),
            size_mm,
            caption: caption.to_owned(),
            align: Align::Center,
        },
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

/// The standard act of `kind` with `block` first.
fn act_with(kind: DocumentKind, block: Block) -> Template {
    let mut t = standard_template(kind);
    t.blocks.insert(0, block);
    t
}

/// A page 100 × 120 mm of `kind` with `blocks`.
fn small_page(kind: DocumentKind, blocks: Vec<Block>) -> Template {
    Template {
        kind,
        page: PageSetup {
            size: PageSize::Custom {
                width_mm: 100.0,
                height_mm: 120.0,
            },
            landscape: false,
            margins_mm: margins(5.0),
        },
        layout: Layout::OnePerPage,
        font_size_pt: 9.0,
        blocks,
    }
}

fn pages(t: &Template, data: &DocumentData, opts: &RenderOptions, dpi: u32) -> Vec<Vec<u8>> {
    assert_eq!(t.validate(), Ok(()));
    render(t, data, opts)
        .expect("renders")
        .png_pages(dpi)
        .expect("png")
}

/// Every code found on a PNG page, decoded and sorted.
fn decode_all(png: &[u8]) -> Vec<String> {
    let page = image::load_from_memory(png).expect("png").to_luma_alpha8();
    let (w, h) = (page.width() as usize, page.height() as usize);
    let mut image = rqrr::PreparedImage::prepare_from_greyscale(w, h, |x, y| {
        let [luma, alpha] = page.get_pixel(x as u32, y as u32).0;
        // On white paper.
        let ink = (255 - u16::from(luma)) * u16::from(alpha) / 255;
        (255 - ink) as u8
    });
    let mut found: Vec<String> = image
        .detect_grids()
        .iter()
        .map(|grid| grid.decode().expect("decodes").1)
        .collect();
    found.sort();
    found
}

fn with_value(kind: DocumentKind, key: &str, value: &str) -> DocumentData {
    let mut data = sample_data(kind);
    data.values.insert(key.to_owned(), value.to_owned());
    data
}

#[test]
fn qr_block_prints_its_link() {
    let t = act_with(
        DocumentKind::IntakeAct,
        qr("{фирма.телеграм}", 25.0, "Мы в Telegram"),
    );
    let data = sample_data(DocumentKind::IntakeAct);
    let doc = render(&t, &data, &RenderOptions::default()).expect("renders");
    let first = &doc.png_pages(300).expect("png")[0];
    let found = decode_all(first);
    assert!(!found.is_empty());
    assert!(found.iter().all(|code| code == TELEGRAM), "{found:?}");
    assert!(doc.text().contains("Мы в Telegram"), "{}", doc.text());
}

/// The captions of the presets at the default 20 mm are wider than the code: they print on one line,
/// centred under it, with room for a quiet zone between them.
#[test]
fn preset_caption_is_one_line_at_the_default_size() {
    let opts = RenderOptions::default();
    let data = sample_data(DocumentKind::IntakeAct);
    for (link, caption) in [
        ("{фирма.гугл_карты}", "Мы на Google Картах"),
        ("{фирма.яндекс_карты}", "Мы на Яндекс Картах"),
    ] {
        let t = small_page(DocumentKind::IntakeAct, vec![qr(link, 20.0, caption)]);
        let doc = render(&t, &data, &opts).expect("renders");
        assert!(doc.text().contains(caption), "{}", doc.text());
        assert_eq!(decode_all(&doc.png_pages(300).expect("png")[0]).len(), 1);
        // A tenth of the side, at least 1 mm: about 2 modules of a typical code.
        assert!(to_typst(&t, &opts).contains("spacing: 2.000mm"));
    }
}

/// A caption wider than the column wraps inside it, whatever the alignment of the block: the other
/// column stays blank.
#[test]
fn caption_stays_in_its_column() {
    let caption = "Отсканируйте код камерой телефона, чтобы написать нам в Telegram";
    let dpi = 203;
    for align in [Align::Left, Align::Center, Align::Right] {
        let mut block = qr("{фирма.телеграм}", 15.0, caption);
        if let BlockBody::Qr { align: a, .. } = &mut block.body {
            *a = align;
        }
        let mut t = small_page(
            DocumentKind::Sticker,
            vec![Block {
                copies: CopyFilter::Both,
                body: BlockBody::Columns {
                    columns: vec![vec![block], vec![]],
                    gap_mm: 2.0,
                },
            }],
        );
        t.page.size = PageSize::Custom {
            width_mm: 60.0,
            height_mm: 60.0,
        };
        t.page.margins_mm = margins(2.0);
        t.font_size_pt = 8.0;
        let opts = RenderOptions {
            label_mm: Some((60.0, 60.0)),
            offset_mm: (0.0, 0.0),
            dpi: Some(dpi),
        };
        let data = sample_data(DocumentKind::Sticker);
        let png = &pages(&t, &data, &opts, dpi)[0];
        assert_eq!(decode_all(png), [TELEGRAM], "{align:?}");
        // The first column ends at 2 + (60 − 2 · 2 − 2) / 2 = 29 mm.
        let page = image::load_from_memory(png).expect("png").to_luma_alpha8();
        let from = (29.5 / 25.4 * f64::from(dpi)).ceil() as u32;
        let ink = (from..page.width())
            .flat_map(|x| (0..page.height()).map(move |y| (x, y)))
            .filter(|&(x, y)| {
                let [luma, alpha] = page.get_pixel(x, y).0;
                alpha > 0 && luma < 128
            })
            .count();
        assert_eq!(ink, 0, "{align:?}");
        assert!(
            render(&t, &data, &opts)
                .expect("renders")
                .text()
                .contains("Telegram")
        );
    }
}

/// The edges of the ink on a PNG page, in mm: (left, right).
fn ink_edges_mm(png: &[u8], dpi: u32) -> (f64, f64) {
    let page = image::load_from_memory(png).expect("png").to_luma_alpha8();
    let columns: Vec<u32> = (0..page.width())
        .filter(|&x| {
            (0..page.height()).any(|y| {
                let [luma, alpha] = page.get_pixel(x, y).0;
                alpha > 0 && luma < 128
            })
        })
        .collect();
    let to_mm = |px: u32| f64::from(px) * 25.4 / f64::from(dpi);
    (
        to_mm(*columns.first().expect("ink")),
        to_mm(*columns.last().expect("ink") + 1),
    )
}

/// A caption wider than the code widens the block, which keeps its alignment in the column.
#[test]
fn wide_caption_block_keeps_its_alignment() {
    let dpi = 300;
    let data = sample_data(DocumentKind::IntakeAct);
    let opts = RenderOptions::default();
    let edges = |align: Align| {
        let mut block = qr("{фирма.гугл_карты}", 15.0, "Мы на Google Картах");
        if let BlockBody::Qr { align: a, .. } = &mut block.body {
            *a = align;
        }
        ink_edges_mm(
            &pages(
                &small_page(DocumentKind::IntakeAct, vec![block]),
                &data,
                &opts,
                dpi,
            )[0],
            dpi,
        )
    };
    // The page is 100 mm wide with margins of 5 mm.
    let (left, right) = edges(Align::Left);
    assert!((5.0..5.6).contains(&left), "{left}");
    assert!(right - left > 15.5 && right < 60.0, "{left}..{right}");
    let (left, right) = edges(Align::Right);
    assert!((94.4..=95.0).contains(&right), "{right}");
    assert!(right - left > 15.5 && left > 40.0, "{left}..{right}");
    let (left, right) = edges(Align::Center);
    assert!(((left + right) / 2.0 - 50.0).abs() < 0.5, "{left}..{right}");
}

/// A link of its own inside a column: its file is named by the block's path there.
#[test]
fn own_link_in_a_column_prints_its_link() {
    let own = "https://example.com/o/{заказ.номер}";
    let t = small_page(
        DocumentKind::IntakeAct,
        vec![
            qr("{фирма.телеграм}", 20.0, ""),
            Block {
                copies: CopyFilter::Both,
                body: BlockBody::Columns {
                    columns: vec![
                        vec![qr("{фирма.яндекс_карты}", 25.0, "")],
                        vec![qr(own, 25.0, "Заказ")],
                    ],
                    gap_mm: 4.0,
                },
            },
        ],
    );
    let opts = RenderOptions::default();
    assert_eq!(
        to_typst(&t, &opts)
            .matches("\"qr-block-1-1-0.svg\"")
            .count(),
        1
    );
    let data = sample_data(DocumentKind::IntakeAct);
    assert_eq!(
        decode_all(&pages(&t, &data, &opts, 300)[0]),
        ["https://example.com/o/1234", TELEGRAM, YANDEX]
    );
}

#[test]
fn custom_link_with_substitution_is_filled_per_document() {
    let t = small_page(
        DocumentKind::IntakeAct,
        vec![qr("https://example.com/o/{заказ.номер}", 30.0, "")],
    );
    let opts = RenderOptions::default();
    let data = sample_data(DocumentKind::IntakeAct);
    assert_eq!(
        decode_all(&pages(&t, &data, &opts, 300)[0]),
        ["https://example.com/o/1234"]
    );
    let other = with_value(DocumentKind::IntakeAct, "order.number", "77");
    assert_eq!(
        decode_all(&pages(&t, &other, &opts, 300)[0]),
        ["https://example.com/o/77"]
    );
}

#[test]
fn non_latin_link_round_trips() {
    let link = "https://t.me/сервис_пример";
    let t = small_page(
        DocumentKind::IntakeAct,
        vec![qr("{фирма.телеграм}", 30.0, "")],
    );
    let data = with_value(DocumentKind::IntakeAct, "firm.telegram", link);
    assert_eq!(
        decode_all(&pages(&t, &data, &RenderOptions::default(), 300)[0]),
        [link]
    );
}

#[test]
fn narrow_column_shrinks_the_code() {
    let mut t = small_page(
        DocumentKind::Sticker,
        vec![Block {
            copies: CopyFilter::Both,
            body: BlockBody::Columns {
                columns: vec![
                    vec![qr("{фирма.телеграм}", 100.0, "")],
                    vec![qr("{фирма.яндекс_карты}", 100.0, "")],
                ],
                gap_mm: 2.0,
            },
        }],
    );
    t.page.size = PageSize::Custom {
        width_mm: 40.0,
        height_mm: 30.0,
    };
    t.page.margins_mm = margins(2.0);
    t.font_size_pt = 7.0;
    let opts = RenderOptions {
        label_mm: Some((40.0, 30.0)),
        offset_mm: (0.0, 0.0),
        dpi: Some(203),
    };
    let data = sample_data(DocumentKind::Sticker);
    assert_eq!(
        decode_all(&pages(&t, &data, &opts, 203)[0]),
        [TELEGRAM, YANDEX]
    );
    // The side is the column's width: (40 − 2 · 2 − 2) / 2 = 17 mm.
    let markup = to_typst(&t, &opts);
    for name in ["firm.telegram", "firm.yandex_maps"] {
        let image = format!("image(\"qr-{name}.svg\", width: ");
        assert_eq!(markup.matches(&image).count(), 1, "{markup}");
        let (_, rest) = markup.split_once(&image).expect("image");
        let (width, _) = rest.split_once("mm").expect("mm");
        let width: f64 = width.parse().expect("number");
        assert!(width <= 17.0 && width > 16.99, "{name}: {width}");
    }
}

#[test]
fn empty_link_prints_nothing_and_takes_no_room() {
    let kind = DocumentKind::IntakeAct;
    let with = act_with(kind, qr("{фирма.телеграм}", 25.0, "Мы в Telegram"));
    let without = standard_template(kind);
    let opts = RenderOptions::default();
    for empty in ["", "  "] {
        let data = with_value(kind, "firm.telegram", empty);
        assert!(
            pages(&with, &data, &opts, 300) == pages(&without, &data, &opts, 300),
            "{empty:?}"
        );
    }
}

#[test]
fn blank_form_prints_no_half_link() {
    let kind = DocumentKind::BlankIntakeAct;
    let data = sample_data(kind);
    let opts = RenderOptions::default();
    let order = small_page(
        kind,
        vec![qr("https://example.com/o/{заказ.номер}", 30.0, "")],
    );
    assert!(decode_all(&pages(&order, &data, &opts, 300)[0]).is_empty());
    let firm = small_page(kind, vec![qr("{фирма.телеграм}", 30.0, "")]);
    assert_eq!(decode_all(&pages(&firm, &data, &opts, 300)[0]), [TELEGRAM]);
}

#[test]
fn one_file_per_name() {
    let own = "https://example.com/own";
    let t = small_page(
        DocumentKind::IntakeAct,
        vec![
            qr("{фирма.телеграм}", 20.0, ""),
            qr("{фирма.телеграм}", 30.0, ""),
            qr(own, 25.0, ""),
        ],
    );
    let opts = RenderOptions::default();
    let markup = to_typst(&t, &opts);
    assert_eq!(markup.matches("image(\"qr-firm.telegram.svg\"").count(), 2);
    assert_eq!(markup.matches("\"qr-block-2.svg\"").count(), 1);
    let data = sample_data(DocumentKind::IntakeAct);
    assert_eq!(
        decode_all(&pages(&t, &data, &opts, 300)[0]),
        [own, TELEGRAM, TELEGRAM]
    );
}

#[test]
fn markup_has_no_link() {
    let t = small_page(
        DocumentKind::IntakeAct,
        vec![qr("https://example.com/секрет", 30.0, "")],
    );
    let markup = to_typst(&t, &RenderOptions::default());
    assert!(!markup.contains("секрет"), "{markup}");
    assert!(!markup.contains("example.com"), "{markup}");
}
