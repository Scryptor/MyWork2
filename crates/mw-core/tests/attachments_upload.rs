//! Загрузка фото и файлов, черновики приёма, обработка фото.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use std::io::Cursor;

use common::*;
use mw_core::attachments::{PhotoError, process_photo};
use mw_core::{AttachmentKind, Core, CoreError, NewOrder};
use mw_domain::permission::Permission;

fn encode(img: image::DynamicImage, format: image::ImageFormat) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, format).unwrap();
    out.into_inner()
}

fn jpeg_bytes(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbImage::from_pixel(w, h, image::Rgb([200, 30, 30]));
    encode(
        image::DynamicImage::ImageRgb8(img),
        image::ImageFormat::Jpeg,
    )
}

/// Вставляет сразу после SOI сегмент APP1 Exif с Orientation = 6.
fn with_exif_orientation_6(jpeg: &[u8]) -> Vec<u8> {
    let mut tiff = Vec::new();
    tiff.extend_from_slice(b"MM\0\x2a\0\0\0\x08");
    tiff.extend_from_slice(&[0, 1]); // одна запись
    tiff.extend_from_slice(&[0x01, 0x12, 0, 3, 0, 0, 0, 1, 0, 6, 0, 0]);
    tiff.extend_from_slice(&[0, 0, 0, 0]); // нет следующего IFD
    let mut app1 = b"Exif\0\0".to_vec();
    app1.extend_from_slice(&tiff);
    let len = u16::try_from(app1.len() + 2).unwrap();
    let mut out = jpeg[..2].to_vec();
    out.extend_from_slice(&[0xFF, 0xE1]);
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(&app1);
    out.extend_from_slice(&jpeg[2..]);
    out
}

fn dims(bytes: &[u8]) -> (u32, u32) {
    let img = image::load_from_memory(bytes).unwrap();
    (img.width(), img.height())
}

#[test]
fn photo_processing_rotates_by_exif_and_strips_metadata() {
    let src = with_exif_orientation_6(&jpeg_bytes(400, 200));
    assert!(src.windows(4).any(|w| w == b"Exif"));
    let p = process_photo(&src).unwrap();
    assert_eq!((p.width, p.height), (200, 400));
    assert_eq!(dims(&p.jpeg), (200, 400));
    assert!(!p.jpeg.windows(4).any(|w| w == b"Exif"));
    assert!(!p.thumb.windows(4).any(|w| w == b"Exif"));
}

#[test]
fn photo_processing_scales_down_and_makes_thumb() {
    let p = process_photo(&jpeg_bytes(6000, 3000)).unwrap();
    assert_eq!((p.width, p.height), (2560, 1280));
    assert_eq!(dims(&p.thumb), (320, 160));
}

#[test]
fn photo_processing_flattens_transparency_on_white() {
    let img = image::RgbaImage::from_pixel(2, 2, image::Rgba([10, 20, 30, 0]));
    let png = encode(
        image::DynamicImage::ImageRgba8(img),
        image::ImageFormat::Png,
    );
    let p = process_photo(&png).unwrap();
    let out = image::load_from_memory(&p.jpeg).unwrap().to_rgb8();
    let px = out.get_pixel(0, 0).0;
    assert!(px.iter().all(|c| *c >= 253), "{px:?}");
}

#[test]
fn photo_processing_refuses_non_images() {
    assert!(matches!(
        process_photo(b"\0\0\0\x18ftypheic\0\0\0\0"),
        Err(PhotoError::Unsupported { .. })
    ));
    assert!(matches!(
        process_photo(b"just text"),
        Err(PhotoError::Unsupported { .. })
    ));
    let mut cut = jpeg_bytes(100, 100);
    cut.truncate(60);
    assert!(matches!(process_photo(&cut), Err(PhotoError::Broken)));
}

fn creator() -> mw_core::Actor {
    actor_with(&[Permission::OrdersCreate, Permission::OrdersEdit])
}

async fn with_dir(t: &TestCore, dir: &std::path::Path) -> Core {
    t.core.clone().with_attachments_dir(dir.to_path_buf())
}

#[tokio::test]
async fn heic_and_broken_uploads_are_refused() -> Result<(), CoreError> {
    for t in cores().await {
        let dir = tempfile::tempdir().unwrap();
        let core = with_dir(&t, dir.path()).await;
        let o = core.create_order(&admin(), sample_new_order()).await?;
        assert_validation(
            core.add_order_attachment(
                &creator(),
                o.id,
                "a.heic",
                b"\0\0\0\x18ftypheic\0\0".to_vec(),
            )
            .await,
            &[("file", "unsupported")],
        );
        assert_validation(
            core.add_order_attachment(&creator(), o.id, "x.jpg", b"garbage garbage".to_vec())
                .await,
            &[("file", "broken")],
        );
        assert!(
            core.list_order_attachments(&admin(), o.id)
                .await?
                .is_empty()
        );
    }
    Ok(())
}

#[tokio::test]
async fn order_uploads_store_photo_and_file() -> Result<(), CoreError> {
    for t in cores().await {
        let dir = tempfile::tempdir().unwrap();
        let core = with_dir(&t, dir.path()).await;
        let o = core.create_order(&admin(), sample_new_order()).await?;
        let photo = core
            .add_order_attachment(&creator(), o.id, "IMG 1.PNG", jpeg_bytes(50, 40))
            .await?;
        assert_eq!(photo.kind, AttachmentKind::Photo);
        assert_eq!(photo.content_type, "image/jpeg");
        assert_eq!((photo.width, photo.height), (Some(50), Some(40)));
        let file = core
            .add_order_attachment(&creator(), o.id, "Акт.PDF", b"%PDF-1.4".to_vec())
            .await?;
        assert_eq!(file.kind, AttachmentKind::File);
        assert_eq!(file.content_type, "application/pdf");
        assert_eq!(file.size, 8);
        assert_eq!(core.list_order_attachments(&admin(), o.id).await?.len(), 2);
        assert_eq!(count_audit(&core, "attachment.added").await, 2);
        let f = core
            .attachment_file(&admin(), photo.id, mw_core::AttachmentVariant::Full)
            .await?;
        assert!(
            f.path
                .starts_with(dir.path().join(format!("attachments/orders/{}", o.id)))
        );
        assert_eq!(f.path.extension().unwrap(), "jpg");
        core.attachment_file(&admin(), photo.id, mw_core::AttachmentVariant::Thumb)
            .await?;
        assert!(matches!(
            core.add_order_attachment(
                &actor_with(&[Permission::OrdersCreate]),
                o.id,
                "a.txt",
                vec![1]
            )
            .await,
            Err(CoreError::Forbidden(_))
        ));
    }
    Ok(())
}

#[tokio::test]
async fn too_large_upload_is_refused() -> Result<(), CoreError> {
    for t in cores().await {
        let dir = tempfile::tempdir().unwrap();
        let core = with_dir(&t, dir.path()).await;
        let o = core.create_order(&admin(), sample_new_order()).await?;
        assert_validation(
            core.add_order_attachment(&creator(), o.id, "big.bin", vec![0; 25 * 1024 * 1024 + 1])
                .await,
            &[("file", "too_large")],
        );
    }
    Ok(())
}

fn files_in(dir: &std::path::Path) -> usize {
    walk(dir)
}

fn walk(dir: &std::path::Path) -> usize {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return 0;
    };
    rd.flatten()
        .map(|e| {
            if e.path().is_dir() {
                walk(&e.path())
            } else {
                1
            }
        })
        .sum()
}

#[tokio::test]
async fn draft_attachments_move_to_the_created_order() -> Result<(), CoreError> {
    for t in cores().await {
        let dir = tempfile::tempdir().unwrap();
        let core = with_dir(&t, dir.path()).await;
        for _ in 0..2 {
            core.add_draft_attachment(&creator(), "k1", "p.jpg", jpeg_bytes(30, 30))
                .await?;
        }
        assert_eq!(
            core.list_draft_attachments(&creator(), "k1").await?.len(),
            2
        );
        assert_validation(
            core.add_draft_attachment(&creator(), "bad key!", "p.jpg", jpeg_bytes(3, 3))
                .await,
            &[("draft_key", "invalid")],
        );
        let o = core
            .create_order(
                &admin(),
                NewOrder {
                    attachment_draft: Some("k1".into()),
                    ..sample_new_order()
                },
            )
            .await?;
        let list = core.list_order_attachments(&admin(), o.id).await?;
        assert_eq!(list.len(), 2);
        assert!(
            core.list_draft_attachments(&creator(), "k1")
                .await?
                .is_empty()
        );
        let order_dir = dir.path().join(format!("attachments/orders/{}", o.id));
        for a in &list {
            assert!(!a.missing, "{}", t.label);
            let f = core
                .attachment_file(&admin(), a.id, mw_core::AttachmentVariant::Full)
                .await?;
            assert!(f.path.starts_with(&order_dir), "{:?}", f.path);
        }
        assert!(!dir.path().join("attachments/drafts/k1").exists());
        assert_eq!(files_in(&order_dir), 4);
    }
    Ok(())
}

#[tokio::test]
async fn draft_limit_is_ten() -> Result<(), CoreError> {
    for t in cores().await {
        let dir = tempfile::tempdir().unwrap();
        let core = with_dir(&t, dir.path()).await;
        for _ in 0..10 {
            core.add_draft_attachment(&creator(), "full", "p.jpg", jpeg_bytes(8, 8))
                .await?;
        }
        assert_validation(
            core.add_draft_attachment(&creator(), "full", "p.jpg", jpeg_bytes(8, 8))
                .await,
            &[("file", "too_many")],
        );
    }
    Ok(())
}

#[tokio::test]
async fn draft_attachments_are_discarded_and_cleaned() -> Result<(), CoreError> {
    for t in cores().await {
        let dir = tempfile::tempdir().unwrap();
        let core = with_dir(&t, dir.path()).await;
        core.add_draft_attachment(&creator(), "k2", "p.jpg", jpeg_bytes(20, 20))
            .await?;
        core.discard_draft(&creator(), "k2").await?;
        core.discard_draft(&creator(), "unknown").await?;
        assert!(
            core.list_draft_attachments(&creator(), "k2")
                .await?
                .is_empty()
        );
        assert_eq!(files_in(&dir.path().join("attachments/drafts")), 0);

        core.add_draft_attachment(&creator(), "k3", "p.jpg", jpeg_bytes(20, 20))
            .await?;
        assert_eq!(core.cleanup_drafts(chrono::Duration::hours(24)).await?, 0);
        t.clock.advance(chrono::Duration::hours(25));
        core.add_draft_attachment(&creator(), "k4", "p.jpg", jpeg_bytes(20, 20))
            .await?;
        assert_eq!(core.cleanup_drafts(chrono::Duration::hours(24)).await?, 1);
        assert!(!dir.path().join("attachments/drafts/k3").exists());
        assert_eq!(
            core.list_draft_attachments(&creator(), "k4").await?.len(),
            1
        );
    }
    Ok(())
}

#[test]
fn bmp_becomes_a_jpeg_photo() {
    let img = image::RgbImage::from_pixel(12, 6, image::Rgb([1, 2, 3]));
    let bmp = encode(image::DynamicImage::ImageRgb8(img), image::ImageFormat::Bmp);
    let p = process_photo(&bmp).unwrap();
    assert_eq!((p.width, p.height), (12, 6));
    assert_eq!(&p.jpeg[..2], &[0xFF, 0xD8]);
}

#[tokio::test]
async fn failed_draft_move_leaves_every_attachment_readable() -> Result<(), CoreError> {
    for t in cores().await {
        let dir = tempfile::tempdir().unwrap();
        let core = with_dir(&t, dir.path()).await;
        let first = core.create_order(&admin(), sample_new_order()).await?;
        for _ in 0..2 {
            core.add_draft_attachment(&creator(), "mv", "p.jpg", jpeg_bytes(20, 20))
                .await?;
        }
        // Папку следующего заказа занимает файл: перенос невозможен.
        let blocked = dir
            .path()
            .join(format!("attachments/orders/{}", first.id + 1));
        std::fs::create_dir_all(blocked.parent().unwrap()).unwrap();
        std::fs::write(&blocked, b"x").unwrap();
        let o = core
            .create_order(
                &admin(),
                NewOrder {
                    attachment_draft: Some("mv".into()),
                    ..sample_new_order()
                },
            )
            .await?;
        assert_eq!(o.id, first.id + 1);
        let list = core.list_order_attachments(&admin(), o.id).await?;
        assert_eq!(list.len(), 2);
        for a in &list {
            assert!(!a.missing, "{}", t.label);
            core.attachment_file(&admin(), a.id, mw_core::AttachmentVariant::Thumb)
                .await?;
        }
    }
    Ok(())
}
