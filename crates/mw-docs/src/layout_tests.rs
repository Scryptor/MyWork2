//! Tests that look at where things land on the laid-out page: the barcode crops and the calibration
//! offset.

use mw_domain::documents::DocumentKind;
use typst::layout::{Frame, FrameItem, Point};
use typst::visualize::ImageKind;

use crate::model::{
    Align, Block, BlockBody, CopyFilter, Layout, Margins, PageSetup, PageSize, Template,
};
use crate::{DocumentData, RenderOptions, Rendered, render, sample_data, standard_template};

const BARCODE: &str = "200000012342";
/// Modules of the whole symbol, the quiet zones included.
const SYMBOL_MODULES: f64 = 113.0;
const EPS_MM: f64 = 1e-3;

/// A printed barcode: the clipping box and the image in it, in mm.
#[derive(Debug)]
struct Crop {
    box_height: f64,
    /// The top of the image below the top of the box (negative: above it).
    image_y: f64,
    image_width: f64,
    image_height: f64,
    /// Heights of the regular and of the guard bars, and of the whole SVG, in the SVG's own mm.
    regular: f64,
    guard: f64,
    svg_height: f64,
}

impl Crop {
    /// The module of the printed bars.
    fn module(&self) -> f64 {
        self.image_width / SYMBOL_MODULES
    }

    fn scale(&self) -> f64 {
        self.image_height / self.svg_height
    }
}

fn crops(doc: &Rendered) -> Vec<Crop> {
    let mut out = Vec::new();
    for page in doc.document().pages() {
        find_crops(&page.frame, &mut out);
    }
    out
}

fn find_crops(frame: &Frame, out: &mut Vec<Crop>) {
    for (_, item) in frame.items() {
        if let FrameItem::Group(group) = item {
            let image = group
                .clip
                .as_ref()
                .and_then(|_| svg_in(&group.frame, Point::zero()));
            match image {
                Some((pos, width, height, svg)) => out.push(Crop {
                    box_height: group.frame.height().to_mm(),
                    image_y: pos.y.to_mm(),
                    image_width: width,
                    image_height: height,
                    regular: bar_heights(&svg).0,
                    guard: bar_heights(&svg).1,
                    svg_height: svg_height(&svg),
                }),
                None => find_crops(&group.frame, out),
            }
        }
    }
}

/// The first SVG image in `frame`: its position (with `offset`), width and height in mm, and source.
fn svg_in(frame: &Frame, offset: Point) -> Option<(Point, f64, f64, String)> {
    frame.items().find_map(|(pos, item)| match item {
        FrameItem::Image(image, size, _) => match image.kind() {
            ImageKind::Svg(svg) => Some((
                offset + *pos,
                size.x.to_mm(),
                size.y.to_mm(),
                String::from_utf8(svg.data().to_vec()).expect("utf-8"),
            )),
            _ => None,
        },
        FrameItem::Group(group) => {
            assert!(group.transform.is_identity());
            svg_in(&group.frame, offset + *pos)
        }
        _ => None,
    })
}

/// (lowest, tallest) bar of the SVG.
fn bar_heights(svg: &str) -> (f64, f64) {
    let heights: Vec<f64> = svg
        .split("<rect")
        .skip(1)
        .map(|tag| attribute(tag, "height"))
        .collect();
    let lowest = heights.iter().copied().fold(f64::INFINITY, f64::min);
    let tallest = heights.iter().copied().fold(0.0, f64::max);
    (lowest, tallest)
}

fn svg_height(svg: &str) -> f64 {
    let key = "viewBox=\"";
    let start = svg.find(key).expect("viewBox") + key.len();
    let view_box = &svg[start..start + svg[start..].find('"').expect("quote")];
    view_box
        .split(' ')
        .nth(3)
        .and_then(|h| h.parse().ok())
        .expect("height")
}

fn attribute(tag: &str, name: &str) -> f64 {
    let key = format!(" {name}=\"");
    let start = tag.find(&key).expect("attribute") + key.len();
    tag[start..start + tag[start..].find('"').expect("quote")]
        .parse()
        .expect("number")
}

/// Every crop shows bars from its top to its bottom, and the guard bars stick out 5 of its own
/// modules below the regular ones.
fn assert_covered(crop: &Crop, what: &str) {
    assert!(
        crop.image_y <= EPS_MM,
        "{what}: bars start below the top: {crop:?}"
    );
    assert!(
        (crop.image_y + crop.image_height - crop.box_height).abs() < EPS_MM,
        "{what}: the image does not end at the bottom: {crop:?}"
    );
    assert!(
        (crop.guard - crop.svg_height).abs() < EPS_MM,
        "{what}: {crop:?}"
    );
    let guard_extension = (crop.guard - crop.regular) * crop.scale();
    assert!(
        (guard_extension - 5.0 * crop.module()).abs() < EPS_MM,
        "{what}: guard bars stick out {guard_extension} mm, not 5 modules: {crop:?}"
    );
}

fn sticker_options(label_mm: Option<(f32, f32)>, dpi: Option<u32>) -> RenderOptions {
    RenderOptions {
        label_mm,
        offset_mm: (0.0, 0.0),
        dpi,
    }
}

#[test]
fn every_standard_barcode_crop_is_covered_and_the_image_is_never_taller_than_the_page() {
    const DPIS: [Option<u32>; 4] = [None, Some(203), Some(300), Some(600)];
    const LABELS: [Option<(f32, f32)>; 7] = [
        None,
        Some((58.0, 40.0)),
        Some((40.0, 30.0)),
        Some((15.0, 15.0)),
        Some((200.0, 200.0)),
        Some((200.0, 15.0)),
        Some((15.0, 200.0)),
    ];
    for kind in DocumentKind::ALL {
        let template = standard_template(kind);
        let data = sample_data(kind);
        let labels: &[Option<(f32, f32)>] = if template.layout == Layout::TwoPerA4 {
            &[None]
        } else {
            &LABELS
        };
        for &label_mm in labels {
            for dpi in DPIS {
                let opts = sticker_options(label_mm, dpi);
                let what = format!("{kind:?} {label_mm:?} {dpi:?}");
                let doc = render(&template, &data, &opts).unwrap_or_else(|e| panic!("{what}: {e}"));
                let crops = crops(&doc);
                let expected = match (data.barcode.is_some(), template.layout) {
                    (false, _) => 0,
                    (true, Layout::TwoPerA4) => 2,
                    (true, Layout::OnePerPage) => 1,
                };
                assert_eq!(crops.len(), expected, "{what}: {crops:?}");
                let page_height = label_mm.map_or_else(
                    || f64::from(template.page.size.size_mm().1),
                    |(_, h)| f64::from(h),
                );
                let margins = template.page.margins_mm;
                let area_height = page_height - f64::from(margins.top + margins.bottom);
                for crop in &crops {
                    assert_covered(crop, &what);
                    // One image serves every copy: its bars are no longer than the page area (twice
                    // the page took hundreds of megabytes of textures on a 600 dpi printer).
                    assert!(crop.regular <= area_height + EPS_MM, "{what}: {crop:?}");
                }
            }
        }
    }
}

fn block(body: BlockBody) -> Block {
    Block {
        copies: CopyFilter::Both,
        body,
    }
}

fn barcode(height_mm: f32) -> Block {
    block(BlockBody::Barcode {
        height_mm,
        digits: false,
        align: Align::Left,
        fill: false,
    })
}

/// A full-width barcode and, in an 18 mm column, a tall one: at 400 dpi their modules are 6 and
/// 2 dots.
fn wide_and_narrow(wide_first: bool) -> Template {
    let narrow = block(BlockBody::Columns {
        columns: vec![vec![barcode(200.0)], vec![]],
        gap_mm: 154.0,
    });
    let wide = barcode(10.0);
    Template {
        kind: DocumentKind::Sticker,
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
        layout: Layout::OnePerPage,
        font_size_pt: 9.0,
        blocks: if wide_first {
            vec![wide, narrow]
        } else {
            vec![narrow, wide]
        },
    }
}

#[test]
fn a_block_with_a_third_of_the_first_module_still_gets_bars_down_to_the_top() {
    let data = DocumentData {
        barcode: Some(BARCODE.to_owned()),
        ..DocumentData::default()
    };
    let dot = 25.4 / 400.0;
    for wide_first in [true, false] {
        let doc = render(
            &wide_and_narrow(wide_first),
            &data,
            &sticker_options(None, Some(400)),
        )
        .expect("renders");
        assert_eq!(doc.page_count(), 1);
        let mut crops = crops(&doc);
        assert_eq!(crops.len(), 2, "{crops:?}");
        if !wide_first {
            crops.reverse();
        }
        let (wide, narrow) = (&crops[0], &crops[1]);
        assert!((wide.module() - 6.0 * dot).abs() < EPS_MM, "{wide:?}");
        assert!((narrow.module() - 2.0 * dot).abs() < EPS_MM, "{narrow:?}");
        assert!(
            (narrow.box_height - (200.0 + 5.0 * narrow.module())).abs() < EPS_MM,
            "{narrow:?}"
        );
        for crop in [wide, narrow] {
            assert_covered(crop, &format!("wide first: {wide_first}"));
        }
    }
}

/// The position of the first shape and of the first text on the first page.
fn first_shape_and_text(doc: &Rendered) -> (Point, Point) {
    fn find(frame: &Frame, offset: Point, text: bool) -> Option<Point> {
        frame.items().find_map(|(pos, item)| match item {
            FrameItem::Shape(..) if !text => Some(offset + *pos),
            FrameItem::Text(..) if text => Some(offset + *pos),
            FrameItem::Group(group) => find(&group.frame, offset + *pos, text),
            _ => None,
        })
    }
    let frame = &doc.document().pages()[0].frame;
    (
        find(frame, Point::zero(), false).expect("a shape"),
        find(frame, Point::zero(), true).expect("a text"),
    )
}

#[test]
fn calibration_offset_moves_the_content_right_and_down() {
    let at = |offset_mm: (f32, f32)| {
        let opts = RenderOptions {
            label_mm: Some((58.0, 40.0)),
            offset_mm,
            dpi: Some(203),
        };
        let template = standard_template(DocumentKind::Calibration);
        first_shape_and_text(&render(&template, &DocumentData::default(), &opts).expect("renders"))
    };
    let (still, moved) = (at((0.0, 0.0)), at((2.0, 3.0)));
    for (still, moved) in [(still.0, moved.0), (still.1, moved.1)] {
        assert!(
            ((moved.x - still.x).to_mm() - 2.0).abs() < EPS_MM,
            "{still:?} → {moved:?}"
        );
        assert!(
            ((moved.y - still.y).to_mm() - 3.0).abs() < EPS_MM,
            "{still:?} → {moved:?}"
        );
    }
}
