//! Наклейки ячеек и шкафов склада: данные, шаблон, права, выгрузка.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use common::{TestCore, actor_with, admin, assert_validation, cores};
use mw_core::documents::{CellStickerRequest, DocumentBatchRequest, DocumentBytes, DocumentOutput};
use mw_core::{CoreError, NewStockRack, TemplateSave};
use mw_docs::{RenderOptions, standard_template};
use mw_domain::document_template::BlockBody;
use mw_domain::documents::DocumentKind;
use mw_domain::pages::PageSelection;
use mw_domain::permission::Permission;

async fn rack(t: &TestCore, shelves: i32, cells: i32) -> i64 {
    t.core
        .create_stock_rack(
            &admin(),
            NewStockRack {
                code: "B".into(),
                label: "Дисплеи".into(),
                shelves,
                cells_per_shelf: cells,
                service_point_id: None,
            },
        )
        .await
        .unwrap()
        .id
}

fn req(rack_id: i64, cell: Option<(i32, i32)>) -> CellStickerRequest {
    CellStickerRequest {
        rack_id,
        cell,
        options: RenderOptions::default(),
        pages: None,
    }
}

#[tokio::test]
async fn a_cell_sticker_carries_its_code() {
    for t in cores().await {
        let id = rack(&t, 3, 5).await;
        let doc = t
            .core
            .render_cell_stickers(&admin(), req(id, Some((2, 4))))
            .await
            .unwrap();
        assert_eq!(doc.page_count(), 1, "{}", t.label);
        let text = doc.text();
        assert!(text.contains("B-2-4"), "{text:?}");
        assert!(text.contains("Дисплеи"), "{text:?}");
        assert_validation(
            t.core
                .render_cell_stickers(&admin(), req(id, Some((4, 1))))
                .await
                .map(|_| ()),
            &[("cell", "no_such_place")],
        );
        assert!(matches!(
            t.core
                .render_cell_stickers(&admin(), req(id + 99, None))
                .await,
            Err(CoreError::NotFound { .. })
        ));
    }
}

#[tokio::test]
async fn rack_stickers_are_the_rack_then_every_cell() {
    for t in cores().await {
        let id = rack(&t, 2, 2).await;
        let doc = t
            .core
            .render_cell_stickers(&admin(), req(id, None))
            .await
            .unwrap();
        assert_eq!(doc.page_count(), 5, "{}", t.label);
        let text = doc.text();
        let at: Vec<_> = ["B", "B-1-1", "B-1-2", "B-2-1", "B-2-2"]
            .iter()
            .map(|code| {
                text.match_indices(code)
                    .map(|(i, _)| i)
                    .next()
                    .unwrap_or_else(|| panic!("{code}: {text:?}"))
            })
            .collect();
        assert!(at.windows(2).all(|w| w[0] < w[1]), "{at:?}");

        let single = rack_one(&t).await;
        let doc = t
            .core
            .render_cell_stickers(&admin(), req(single, None))
            .await
            .unwrap();
        assert_eq!(doc.page_count(), 1);
        assert!(doc.text().contains("ONE"));

        let second = CellStickerRequest {
            pages: Some(PageSelection::parse("2").unwrap()),
            ..req(id, None)
        };
        let file = t
            .core
            .export_cell_stickers(&admin(), second, DocumentOutput::Png { dpi: 110 })
            .await
            .unwrap();
        assert_eq!(file.page_count, 1);
        assert!(matches!(file.bytes, DocumentBytes::Png(ref p) if p.len() == 1));
        let all = t
            .core
            .export_cell_stickers(&admin(), req(id, None), DocumentOutput::Pdf)
            .await
            .unwrap();
        assert_eq!(all.page_count, 5);
    }
}

async fn rack_one(t: &TestCore) -> i64 {
    t.core
        .create_stock_rack(
            &admin(),
            NewStockRack {
                code: "ONE".into(),
                label: "Один".into(),
                shelves: 1,
                cells_per_shelf: 1,
                service_point_id: None,
            },
        )
        .await
        .unwrap()
        .id
}

#[tokio::test]
async fn single_cell_rack_has_one_sticker_with_the_rack_code() {
    for t in cores().await {
        let id = rack_one(&t).await;
        let doc = t
            .core
            .render_cell_stickers(&admin(), req(id, None))
            .await
            .unwrap();
        assert_eq!(doc.page_count(), 1, "{}", t.label);
        assert!(doc.text().contains("ONE"));
        let cell = t
            .core
            .render_cell_stickers(&admin(), req(id, Some((1, 1))))
            .await
            .unwrap();
        assert!(cell.text().contains("ONE"));
    }
}

#[tokio::test]
async fn stickers_need_stock_view() {
    for t in cores().await {
        let id = rack(&t, 1, 2).await;
        let nobody = actor_with(&[]);
        assert!(matches!(
            t.core
                .export_cell_stickers(&nobody, req(id, None), DocumentOutput::Pdf)
                .await,
            Err(CoreError::Forbidden(Permission::StockView))
        ));
        let viewer = actor_with(&[Permission::StockView]);
        t.core
            .export_cell_stickers(&viewer, req(id, None), DocumentOutput::Pdf)
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn a_saved_cell_template_is_used() {
    for t in cores().await {
        let id = rack(&t, 3, 5).await;
        let mut template = standard_template(DocumentKind::CellSticker);
        let BlockBody::Text { text, .. } = &mut template.blocks[0].body else {
            panic!("text first");
        };
        *text = "Ячейка {ячейка.код}".into();
        t.core
            .save_document_template(
                &admin(),
                DocumentKind::CellSticker,
                TemplateSave {
                    template,
                    version: 0,
                },
            )
            .await
            .unwrap();
        let doc = t
            .core
            .render_cell_stickers(&admin(), req(id, Some((2, 4))))
            .await
            .unwrap();
        assert!(doc.text().contains("Ячейка B-2-4"), "{:?}", doc.text());
    }
}

#[tokio::test]
async fn cell_sticker_is_not_for_order_batches() {
    for t in cores().await {
        assert_validation(
            t.core
                .export_documents(
                    &admin(),
                    DocumentBatchRequest {
                        kind: DocumentKind::CellSticker,
                        order_ids: vec![1],
                        options: RenderOptions::default(),
                    },
                )
                .await,
            &[("kind", "not_allowed")],
        );
    }
}
