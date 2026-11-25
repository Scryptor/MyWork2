//! The examples of the «Справка по коду» dialog (`typst-snippets.json` of the interface) are real code:
//! each of them must render.

use mw_docs::model::{
    Block, BlockBody, CopyFilter, Layout, Margins, PageSetup, PageSize, Template,
};
use mw_docs::{DocumentData, RenderOptions, Rendered, render, sample_data};
use mw_domain::documents::DocumentKind;
use std::sync::Arc;

const SNIPPETS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../apps/ui/src/lib/templates/typst-snippets.json"
));

fn template(code: &str) -> Template {
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
        layout: Layout::TwoPerA4,
        font_size_pt: 9.0,
        blocks: vec![Block {
            copies: CopyFilter::Both,
            body: BlockBody::TypstCode {
                code: code.to_owned(),
            },
        }],
    }
}

fn render_code(code: &str, data: &DocumentData) -> Rendered {
    render(&template(code), data, &RenderOptions::default())
        .unwrap_or_else(|e| panic!("`{code}` does not render: {e}"))
}

/// Every QR code found on the pages, decoded (a QR code is a vector picture, so it is read from pixels).
#[allow(clippy::expect_used)]
fn qr_codes(rendered: &Rendered) -> Vec<String> {
    let mut found = Vec::new();
    for png in rendered.png_pages(300).expect("png") {
        let page = image::load_from_memory(&png).expect("png").to_luma_alpha8();
        let (w, h) = (page.width() as usize, page.height() as usize);
        let mut image = rqrr::PreparedImage::prepare_from_greyscale(w, h, |x, y| {
            let [luma, alpha] = page.get_pixel(x as u32, y as u32).0;
            let ink = (255 - u16::from(luma)) * u16::from(alpha) / 255;
            (255 - ink) as u8
        });
        for grid in image.detect_grids() {
            found.push(grid.decode().expect("decodes").1);
        }
    }
    found
}

/// A sample value; the example is checked against what the sample data really prints.
fn sample<'a>(data: &'a DocumentData, key: &str) -> &'a str {
    let value = data.values.get(key).unwrap_or_else(|| panic!("no {key}"));
    assert!(!value.is_empty(), "the sample {key} is empty");
    value
}

#[test]
fn every_help_example_renders() {
    let json: serde_json::Value =
        serde_json::from_str(SNIPPETS).unwrap_or_else(|e| panic!("json: {e}"));
    assert!(json["badges"].is_array(), "the key `badges` is a list");
    let examples = json["examples"].as_array().expect("examples is a list");
    let code_of = |id: &str| -> &str {
        examples
            .iter()
            .find(|e| e["id"] == id)
            .and_then(|e| e["code"].as_str())
            .unwrap_or_else(|| panic!("no example {id}"))
    };
    let ids: Vec<_> = examples.iter().filter_map(|e| e["id"].as_str()).collect();
    assert_eq!(
        ids,
        ["value", "condition", "table", "qr", "space", "copy"],
        "the examples changed: check each one here"
    );
    let data = sample_data(DocumentKind::IntakeAct);

    let text = render_code(code_of("value"), &data).text();
    assert!(
        text.contains(sample(&data, "order.number")),
        "value: {text:?}"
    );

    let text = render_code(code_of("condition"), &data).text();
    assert!(
        text.contains("Предоплата") && text.contains(sample(&data, "order.prepayment")),
        "condition: {text:?}"
    );
    // An empty prepayment prints nothing.
    let mut empty = data.clone();
    empty
        .values
        .insert("order.prepayment".to_owned(), String::new());
    let text = render_code(code_of("condition"), &empty).text();
    assert!(!text.contains("Предоплата"), "condition (empty): {text:?}");

    let text = render_code(code_of("table"), &data).text();
    for want in [
        "Устройство",
        "Серийный номер",
        sample(&data, "order.device"),
        sample(&data, "order.serial"),
    ] {
        assert!(text.contains(want), "table lacks {want:?}: {text:?}");
    }

    let codes = qr_codes(&render_code(code_of("qr"), &data));
    assert!(!codes.is_empty(), "qr: no QR code printed");
    assert!(
        codes.iter().all(|c| c == sample(&data, "link.telegram")),
        "qr: {codes:?}"
    );
    let unknown = code_of("qr").replace("link.telegram", "link.nope");
    assert!(
        qr_codes(&render_code(&unknown, &data)).is_empty(),
        "an unknown QR name must print nothing"
    );

    // `space` prints nothing: it only has to render.
    render_code(code_of("space"), &data);

    // TwoPerA4 prints both copies; only the client one has the line.
    let text = render_code(code_of("copy"), &data).text();
    assert_eq!(
        text.matches("Этот экземпляр остаётся у вас").count(),
        1,
        "copy: {text:?}"
    );
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

/// The badges of «Вставка в Typst-код»: the markup as a block, the code inside `#{ … }`, every one on a
/// sample with a logo and one without.
#[test]
fn every_badge_renders() {
    let json: serde_json::Value =
        serde_json::from_str(SNIPPETS).unwrap_or_else(|e| panic!("json: {e}"));
    let badges = json["badges"].as_array().expect("badges is a list");
    let ids: Vec<_> = badges.iter().filter_map(|b| b["id"].as_str()).collect();
    assert_eq!(
        ids,
        [
            "order_number",
            "client_name",
            "client_phone",
            "device",
            "received",
            "logo",
            "space",
            "client_copy",
            "service_copy"
        ],
        "the badges changed: check each one here"
    );
    let without_logo = DocumentData {
        logo: None,
        ..sample_data(DocumentKind::IntakeAct)
    };
    let with_logo = DocumentData {
        logo: Some(logo()),
        ..without_logo.clone()
    };
    let samples = [("without a logo", without_logo), ("with a logo", with_logo)];
    let qr = "#qr-image(\"link.telegram\", 20mm)";
    for (_, data) in &samples {
        for badge in badges {
            let markup = badge["markup"].as_str().expect("markup");
            let code = badge["code"].as_str().expect("code");
            assert_eq!(
                markup,
                format!("#{code}"),
                "the code is the markup without #"
            );
            render_code(markup, data);
            render_code(&format!("#{{ {code} }}"), data);
        }
        render_code(qr, data);
        let inside = format!("#{{ {} }}", &qr[1..]);
        render_code(&inside, data);
    }
}
