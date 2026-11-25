//! The examples of the «Справка по коду» dialog (`typst-snippets.json` of the interface) are real code:
//! each of them must render.

use mw_docs::model::{
    Align, Block, BlockBody, CopyFilter, Layout, Margins, PageSetup, PageSize, Template,
};
use mw_docs::{RenderOptions, render, sample_data};
use mw_domain::documents::DocumentKind;

const SNIPPETS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../apps/ui/src/lib/templates/typst-snippets.json"
));

fn template(code: &str) -> Template {
    let block = |body| Block {
        copies: CopyFilter::Both,
        body,
    };
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
        blocks: vec![
            block(BlockBody::Text {
                text: "Пример".to_owned(),
                size_pt: None,
                bold: false,
                align: Align::Left,
                one_line: false,
                frame: false,
                numbered: false,
            }),
            block(BlockBody::TypstCode {
                code: code.to_owned(),
            }),
        ],
    }
}

#[test]
fn every_help_example_renders() {
    let json: serde_json::Value =
        serde_json::from_str(SNIPPETS).unwrap_or_else(|e| panic!("json: {e}"));
    let examples = json["examples"].as_array().expect("examples is a list");
    assert!(!examples.is_empty(), "no examples");
    assert!(json["badges"].is_array(), "the key `badges` is a list");
    let data = sample_data(DocumentKind::IntakeAct);
    for example in examples {
        let id = example["id"].as_str().expect("id");
        let code = example["code"].as_str().expect("code");
        let rendered = render(&template(code), &data, &RenderOptions::default())
            .unwrap_or_else(|e| panic!("example {id} does not render: {e}"));
        assert!(rendered.page_count() > 0, "example {id} has no page");
        assert!(
            !rendered.text().trim().is_empty(),
            "example {id} printed nothing"
        );
    }
}
