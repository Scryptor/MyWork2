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

/// Заказ из черновика `half`, перенос которого в папку заказа прервался на втором файле:
/// первое вложение уже в папке заказа, второе — ещё в `attachments/drafts/half`, но тоже
/// принадлежит заказу. Возвращает номер заказа и пути файлов второго вложения.
async fn half_moved_draft(
    core: &Core,
    dir: &std::path::Path,
) -> Result<(i64, Vec<std::path::PathBuf>), CoreError> {
    let first = core.create_order(&admin(), sample_new_order()).await?;
    core.add_draft_attachment(&creator(), "half", "a.jpg", jpeg_bytes(20, 20))
        .await?;
    let b = core
        .add_draft_attachment(&creator(), "half", "b.jpg", jpeg_bytes(20, 20))
        .await?;
    let b_path = core
        .attachment_file(&admin(), b.id, mw_core::AttachmentVariant::Full)
        .await?
        .path;
    // В папке следующего заказа имя второго файла занято непустой папкой: он не перенесётся.
    let taken = dir
        .join(format!("attachments/orders/{}", first.id + 1))
        .join(b_path.file_name().unwrap())
        .join("inner");
    std::fs::create_dir_all(&taken).unwrap();
    let o = core
        .create_order(
            &admin(),
            NewOrder {
                attachment_draft: Some("half".into()),
                ..sample_new_order()
            },
        )
        .await?;
    assert_eq!(o.id, first.id + 1);
    let paths = vec![
        b_path.clone(),
        std::path::PathBuf::from(format!("{}.thumb.jpg", b_path.display())),
    ];
    for p in &paths {
        assert!(p.starts_with(dir.join("attachments/drafts/half")), "{p:?}");
        assert!(p.exists(), "{p:?}");
    }
    let moved = core.list_order_attachments(&admin(), o.id).await?;
    assert_eq!(moved.len(), 2);
    Ok((o.id, paths))
}

/// Все вложения заказа на месте: основной файл и миниатюра.
async fn assert_order_files_kept(core: &Core, order_id: i64, left_in_draft: &[std::path::PathBuf]) {
    for p in left_in_draft {
        assert!(p.exists(), "{p:?}");
    }
    for a in core
        .list_order_attachments(&admin(), order_id)
        .await
        .unwrap()
    {
        assert!(!a.missing, "{}", a.name);
    }
}

#[tokio::test]
async fn discarding_a_half_moved_draft_keeps_the_files_of_the_order() -> Result<(), CoreError> {
    for t in cores().await {
        let dir = tempfile::tempdir().unwrap();
        let core = with_dir(&t, dir.path()).await;
        let (order_id, kept) = half_moved_draft(&core, dir.path()).await?;
        // «Отмена» после «Принять» без ответа: ключ тот же; фото, добавленное под ним после заказа.
        let c = core
            .add_draft_attachment(&creator(), "half", "c.jpg", jpeg_bytes(20, 20))
            .await?;
        let c_path = core
            .attachment_file(&admin(), c.id, mw_core::AttachmentVariant::Full)
            .await?
            .path;
        core.discard_draft(&creator(), "half").await?;
        assert!(!c_path.exists(), "{}", t.label);
        assert!(
            !std::path::PathBuf::from(format!("{}.thumb.jpg", c_path.display())).exists(),
            "{}",
            t.label
        );
        assert_order_files_kept(&core, order_id, &kept).await;
    }
    Ok(())
}

#[tokio::test]
async fn cleaning_a_half_moved_draft_keeps_the_files_of_the_order() -> Result<(), CoreError> {
    for t in cores().await {
        let dir = tempfile::tempdir().unwrap();
        let core = with_dir(&t, dir.path()).await;
        let (order_id, kept) = half_moved_draft(&core, dir.path()).await?;
        let c = core
            .add_draft_attachment(&creator(), "half", "c.jpg", jpeg_bytes(20, 20))
            .await?;
        let c_path = core
            .attachment_file(&admin(), c.id, mw_core::AttachmentVariant::Full)
            .await?
            .path;
        t.clock.advance(chrono::Duration::hours(25));
        assert_eq!(core.cleanup_drafts(chrono::Duration::hours(24)).await?, 1);
        assert!(!c_path.exists(), "{}", t.label);
        assert_order_files_kept(&core, order_id, &kept).await;
    }
    Ok(())
}

/// Приёмщик без права правки заказов подписывает и убирает фото своего приёма, но не фото
/// заказов; у черновика хватает любого из двух прав.
#[tokio::test]
async fn draft_photos_are_edited_with_either_right_order_photos_with_orders_edit()
-> Result<(), CoreError> {
    for t in cores().await {
        let dir = tempfile::tempdir().unwrap();
        let core = with_dir(&t, dir.path()).await;
        let clerk = actor_with(&[Permission::OrdersCreate]);
        let editor = actor_with(&[Permission::OrdersEdit]);
        let nobody = actor_with(&[Permission::ClientsView]);
        let forbidden = |r: Result<_, CoreError>| matches!(r, Err(CoreError::Forbidden(_)));

        let d = core
            .add_draft_attachment(&clerk, "own", "p.jpg", jpeg_bytes(20, 20))
            .await?;
        let captioned = core
            .update_attachment_caption(&clerk, d.id, "Скол".into())
            .await?;
        assert_eq!(captioned.caption, "Скол", "{}", t.label);
        let by_editor = core
            .update_attachment_caption(&editor, d.id, "Скол в углу".into())
            .await?;
        assert_eq!(by_editor.caption, "Скол в углу", "{}", t.label);
        assert!(forbidden(
            core.update_attachment_caption(&nobody, d.id, "x".into())
                .await
                .map(|_| ())
        ));
        assert!(forbidden(core.delete_attachment(&nobody, d.id).await));
        core.delete_attachment(&clerk, d.id).await?;
        let e = core
            .add_draft_attachment(&clerk, "own", "p.jpg", jpeg_bytes(20, 20))
            .await?;
        core.delete_attachment(&editor, e.id).await?;
        assert!(core.list_draft_attachments(&clerk, "own").await?.is_empty());

        let o = core.create_order(&admin(), sample_new_order()).await?;
        let p = core
            .add_order_attachment(&creator(), o.id, "p.jpg", jpeg_bytes(20, 20))
            .await?;
        assert!(forbidden(
            core.update_attachment_caption(&clerk, p.id, "x".into())
                .await
                .map(|_| ())
        ));
        assert!(forbidden(core.delete_attachment(&clerk, p.id).await));
        core.update_attachment_caption(&editor, p.id, "x".into())
            .await?;
        core.delete_attachment(&editor, p.id).await?;
    }
    Ok(())
}

/// Узнанное по сигнатуре, но не из пяти форматов фото, и не картинки вовсе сохраняются файлом;
/// отказ — только HEIC и нечитаемые байты под именем картинки.
#[tokio::test]
async fn only_the_five_photo_formats_become_photos_the_rest_are_files() -> Result<(), CoreError> {
    let mut tiff = b"II*\0\x08\0\0\0".to_vec();
    tiff.extend_from_slice(&[0; 32]);
    let avif = b"\0\0\0\x1cftypavif\0\0\0\0avifmif1miaf".to_vec();
    let avif_mif1 = b"\0\0\0\x1cftypmif1\0\0\0\0mif1avifmiaf".to_vec();
    let files: [(&str, Vec<u8>, &str); 5] = [
        ("scan.tif", tiff.clone(), "image/tiff"),
        (
            "BMW.txt",
            "BMW X5, 2019, стекло заказано".as_bytes().to_vec(),
            "text/plain",
        ),
        ("parts.csv", b"P1,P2,P3\n1,2,3\n".to_vec(), "text/csv"),
        ("a.avif", avif, "image/avif"),
        ("b.avif", avif_mif1, "image/avif"),
    ];
    for t in cores().await {
        let dir = tempfile::tempdir().unwrap();
        let core = with_dir(&t, dir.path()).await;
        let o = core.create_order(&admin(), sample_new_order()).await?;
        for (name, bytes, content_type) in &files {
            let a = core
                .add_order_attachment(&creator(), o.id, name, bytes.clone())
                .await
                .unwrap_or_else(|e| panic!("{}: {name}: {e:?}", t.label));
            assert_eq!(a.kind, AttachmentKind::File, "{name}");
            assert_eq!(a.name, *name);
            assert_eq!(a.content_type, *content_type, "{name}");
            assert_eq!(a.size, i64::try_from(bytes.len()).unwrap(), "{name}");
        }
        // Под именем картинки нечитаемое — отказ, а не файл.
        assert_validation(
            core.add_order_attachment(&creator(), o.id, "scan.jpg", tiff.clone())
                .await,
            &[("file", "broken")],
        );
        assert_validation(
            core.add_order_attachment(&creator(), o.id, "BMW.bmp", b"BMW X5".to_vec())
                .await,
            &[("file", "broken")],
        );
        assert_validation(
            core.add_order_attachment(
                &creator(),
                o.id,
                "photo.bin",
                b"\0\0\0\x18ftypheix\0\0\0\0".to_vec(),
            )
            .await,
            &[("file", "unsupported")],
        );
    }
    Ok(())
}

#[test]
fn heic_is_told_apart_from_avif() {
    for brand in [b"heic", b"heix", b"hevc", b"heim", b"heis"] {
        let mut bytes = b"\0\0\0\x18ftyp".to_vec();
        bytes.extend_from_slice(brand);
        bytes.extend_from_slice(&[0; 8]);
        assert_eq!(
            process_photo(&bytes),
            Err(PhotoError::Unsupported {
                format: Some("heic".into())
            }),
            "{}",
            String::from_utf8_lossy(brand)
        );
    }
    for brand in [b"mif1", b"msf1", b"avif"] {
        let mut bytes = b"\0\0\0\x18ftyp".to_vec();
        bytes.extend_from_slice(brand);
        bytes.extend_from_slice(&[0; 8]);
        assert_ne!(
            process_photo(&bytes),
            Err(PhotoError::Unsupported {
                format: Some("heic".into())
            }),
            "{}",
            String::from_utf8_lossy(brand)
        );
    }
}

/// Обработанное фото — JPEG, и имя его говорит так же.
#[tokio::test]
async fn processed_photo_names_end_in_jpg() -> Result<(), CoreError> {
    for t in cores().await {
        let dir = tempfile::tempdir().unwrap();
        let core = with_dir(&t, dir.path()).await;
        for (sent, kept) in [
            ("IMG 1.PNG", "IMG 1.jpg"),
            ("photo", "photo.jpg"),
            ("a.b.webp", "a.b.jpg"),
            ("снимок.jpeg", "снимок.jpg"),
        ] {
            let a = core
                .add_draft_attachment(&creator(), "names", sent, jpeg_bytes(10, 10))
                .await?;
            assert_eq!(a.kind, AttachmentKind::Photo);
            assert_eq!(a.name, kept, "{}", t.label);
        }
    }
    Ok(())
}

/// Срок черновика — сутки настоящего времени, а не по местным часам: в ночь перевода часов
/// местные сутки на час длиннее или короче.
#[tokio::test]
async fn draft_age_is_measured_in_real_hours_across_a_clock_change() -> Result<(), CoreError> {
    use chrono::TimeZone;
    for (label, db) in mw_db::testing::all_backends().await {
        // 2026-10-25 в 01:00 UTC Берлин переводит часы с 03:00 на 02:00.
        let clock = std::sync::Arc::new(mw_core::FixedClock::new(
            chrono::Utc
                .with_ymd_and_hms(2026, 10, 24, 12, 0, 0)
                .unwrap(),
        ));
        let dir = tempfile::tempdir().unwrap();
        let core = Core::open(db, clock.clone(), "Europe/Berlin")
            .await?
            .with_attachments_dir(dir.path().to_path_buf());
        core.add_draft_attachment(&creator(), "dst", "p.jpg", jpeg_bytes(10, 10))
            .await?;
        // 23,5 часа спустя по местным часам прошло 22,5 часа: черновик ещё живой.
        clock.set(
            chrono::Utc
                .with_ymd_and_hms(2026, 10, 25, 11, 30, 0)
                .unwrap(),
        );
        assert_eq!(
            core.cleanup_drafts(chrono::Duration::hours(24)).await?,
            0,
            "{label}"
        );
        // 24,5 часа спустя по местным часам прошло 23,5 часа: срок вышел.
        clock.set(
            chrono::Utc
                .with_ymd_and_hms(2026, 10, 25, 12, 30, 0)
                .unwrap(),
        );
        assert_eq!(
            core.cleanup_drafts(chrono::Duration::hours(24)).await?,
            1,
            "{label}"
        );
    }
    Ok(())
}

/// Последнее фото ушло из черновика (удалением или отменой) — папки черновика больше нет.
#[tokio::test]
async fn draft_folder_goes_away_with_its_last_file() -> Result<(), CoreError> {
    for t in cores().await {
        let dir = tempfile::tempdir().unwrap();
        let core = with_dir(&t, dir.path()).await;
        let drafts = dir.path().join("attachments/drafts");
        let a = core
            .add_draft_attachment(&creator(), "solo", "p.jpg", jpeg_bytes(20, 20))
            .await?;
        let b = core
            .add_draft_attachment(&creator(), "solo", "q.jpg", jpeg_bytes(20, 20))
            .await?;
        core.delete_attachment(&creator(), a.id).await?;
        assert!(drafts.join("solo").exists(), "{}: ещё есть файл", t.label);
        core.delete_attachment(&creator(), b.id).await?;
        assert!(!drafts.join("solo").exists(), "{}", t.label);

        core.add_draft_attachment(&creator(), "gone", "p.jpg", jpeg_bytes(20, 20))
            .await?;
        core.discard_draft(&creator(), "gone").await?;
        assert!(!drafts.join("gone").exists(), "{}", t.label);
    }
    Ok(())
}

/// HEIF с основным брендом `mif1` и совместимым `heic`/`heix` — тот же отказ, что у HEIC;
/// без таких брендов (AVIF) — файл как есть.
#[tokio::test]
async fn heif_with_compatible_heic_brand_is_refused_like_heic() -> Result<(), CoreError> {
    fn ftyp(major: &[u8; 4], compat: &[&[u8; 4]]) -> Vec<u8> {
        let mut b = Vec::new();
        let size = u32::try_from(16 + 4 * compat.len()).unwrap();
        b.extend_from_slice(&size.to_be_bytes());
        b.extend_from_slice(b"ftyp");
        b.extend_from_slice(major);
        b.extend_from_slice(&[0; 4]);
        for c in compat {
            b.extend_from_slice(*c);
        }
        b.extend_from_slice(&[0; 16]);
        b
    }
    for compat in [
        vec![b"mif1", b"heic"],
        vec![b"mif1", b"heix"],
        vec![b"mif1", b"miaf", b"heic"],
    ] {
        assert_eq!(
            process_photo(&ftyp(b"mif1", &compat)),
            Err(PhotoError::Unsupported {
                format: Some("heic".into())
            })
        );
    }
    assert_ne!(
        process_photo(&ftyp(b"mif1", &[b"mif1", b"avif", b"miaf"])),
        Err(PhotoError::Unsupported {
            format: Some("heic".into())
        })
    );
    for t in cores().await {
        let dir = tempfile::tempdir().unwrap();
        let core = with_dir(&t, dir.path()).await;
        let o = core.create_order(&admin(), sample_new_order()).await?;
        assert_validation(
            core.add_order_attachment(
                &creator(),
                o.id,
                "a.heic",
                ftyp(b"mif1", &[b"mif1", b"heic"]),
            )
            .await,
            &[("file", "unsupported")],
        );
    }
    Ok(())
}
