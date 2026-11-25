//! Логотип фирмы: загрузка, замена, обработка картинки, права, отделение от вложений заказов.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use std::io::Cursor;

use chrono::Duration;
use common::*;
use mw_core::documents::{DocumentBytes, DocumentOutput, DocumentRequest};
use mw_core::{AttachmentVariant, Core, CoreError};
use mw_docs::RenderOptions;
use mw_domain::documents::DocumentKind;
use mw_domain::permission::Permission;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

fn encode(img: image::DynamicImage, format: image::ImageFormat) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, format).unwrap();
    out.into_inner()
}

fn png_rgba(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(w, h, image::Rgba([10, 20, 30, 0]));
    encode(
        image::DynamicImage::ImageRgba8(img),
        image::ImageFormat::Png,
    )
}

fn jpeg(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbImage::from_pixel(w, h, image::Rgb([200, 30, 30]));
    encode(
        image::DynamicImage::ImageRgb8(img),
        image::ImageFormat::Jpeg,
    )
}

fn with_dir(t: &TestCore) -> (Core, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    (
        t.core
            .clone()
            .with_attachments_dir(dir.path().to_path_buf()),
        dir,
    )
}

async fn logo_rows(core: &Core) -> Vec<i64> {
    mw_db::entity::attachment::Entity::find()
        .filter(mw_db::entity::attachment::Column::Kind.eq("logo"))
        .all(core.db())
        .await
        .unwrap()
        .into_iter()
        .map(|m| m.id)
        .collect()
}

async fn png_of(core: &Core) -> Vec<u8> {
    let file = core.firm_logo_file(&admin()).await.unwrap().unwrap();
    std::fs::read(file.path).unwrap()
}

fn logo_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    match std::fs::read_dir(dir.join("attachments/logo")) {
        Ok(rd) => rd.map(|e| e.unwrap().path()).collect(),
        Err(_) => Vec::new(),
    }
}

#[tokio::test]
async fn upload_replaces_and_keeps_alpha() {
    for t in cores().await {
        let (core, dir) = with_dir(&t);
        assert_eq!(core.firm_logo(&admin()).await.unwrap(), None, "{}", t.label);
        assert!(core.firm_logo_file(&admin()).await.unwrap().is_none());
        let info = core
            .set_firm_logo(&admin(), png_rgba(64, 32))
            .await
            .unwrap();
        assert_eq!((info.width, info.height), (64, 32), "{}", t.label);
        let files = logo_files(dir.path());
        assert_eq!(files.len(), 1);
        let img = image::open(&files[0]).unwrap();
        assert!(img.color().has_alpha());
        assert_eq!(core.firm_logo(&admin()).await.unwrap(), Some(info.clone()));
        let png = png_of(&core).await;
        assert_eq!(image::guess_format(&png).unwrap(), image::ImageFormat::Png);
        let file = core.firm_logo_file(&admin()).await.unwrap().unwrap();
        assert_eq!(file.content_type, "image/png");
        assert_eq!(file.path, files[0]);

        let second = core.set_firm_logo(&admin(), jpeg(40, 40)).await.unwrap();
        assert_eq!((second.width, second.height), (40, 40));
        let files = logo_files(dir.path());
        assert_eq!(files.len(), 1, "{}: старый файл удалён", t.label);
        assert!(!image::open(&files[0]).unwrap().color().has_alpha());
        assert_eq!(count_audit(&core, "settings.logo_updated").await, 2);
        let rows = logo_rows(&core).await.len();
        assert_eq!(rows, 1, "{}: строка одна", t.label);
    }
}

#[tokio::test]
async fn big_logo_is_downscaled() {
    for t in cores().await {
        let (core, _dir) = with_dir(&t);
        let big = image::RgbImage::from_pixel(3000, 1000, image::Rgb([1, 2, 3]));
        let bytes = encode(image::DynamicImage::ImageRgb8(big), image::ImageFormat::Png);
        let info = core.set_firm_logo(&admin(), bytes).await.unwrap();
        assert_eq!((info.width, info.height), (1200, 400), "{}", t.label);
        let png = png_of(&core).await;
        let img = image::load_from_memory(&png).unwrap();
        assert_eq!((img.width(), img.height()), (1200, 400));
        assert_eq!(info.size, i64::try_from(png.len()).unwrap());
    }
}

#[tokio::test]
async fn odd_images_become_png() {
    for t in cores().await {
        let (core, _dir) = with_dir(&t);
        let rgba16 = image::ImageBuffer::<image::Rgba<u16>, _>::from_pixel(
            8,
            8,
            image::Rgba([60000, 100, 200, 30000]),
        );
        let bytes = encode(
            image::DynamicImage::ImageRgba16(rgba16),
            image::ImageFormat::Png,
        );
        core.set_firm_logo(&admin(), bytes).await.unwrap();
        let png = png_of(&core).await;
        let img = image::load_from_memory(&png).unwrap();
        assert_eq!(img.color(), image::ColorType::Rgba8, "{}", t.label);

        let gray = image::GrayImage::from_pixel(8, 8, image::Luma([90]));
        let bytes = encode(
            image::DynamicImage::ImageLuma8(gray),
            image::ImageFormat::Jpeg,
        );
        core.set_firm_logo(&admin(), bytes).await.unwrap();
        let png = png_of(&core).await;
        assert_eq!(image::guess_format(&png).unwrap(), image::ImageFormat::Png);
        let img = image::load_from_memory(&png).unwrap();
        assert_eq!(img.color(), image::ColorType::L8, "{}", t.label);
    }
}

#[tokio::test]
async fn not_an_image_is_rejected() {
    for t in cores().await {
        let (core, dir) = with_dir(&t);
        assert_validation(
            core.set_firm_logo(&admin(), b"hello".to_vec()).await,
            &[("logo", "unsupported")],
        );
        let mut cut = png_rgba(64, 64);
        cut.truncate(cut.len() / 2);
        assert_validation(
            core.set_firm_logo(&admin(), cut).await,
            &[("logo", "unsupported")],
        );
        // GIF узнаётся, но логотипом не бывает.
        let gif = encode(
            image::DynamicImage::ImageRgba8(image::RgbaImage::new(4, 4)),
            image::ImageFormat::Gif,
        );
        assert_validation(
            core.set_firm_logo(&admin(), gif).await,
            &[("logo", "unsupported")],
        );
        assert_validation(
            core.set_firm_logo(&admin(), vec![0; 5 * 1024 * 1024 + 1])
                .await,
            &[("logo", "too_large")],
        );
        assert!(
            core.firm_logo(&admin()).await.unwrap().is_none(),
            "{}",
            t.label
        );
        assert!(logo_files(dir.path()).is_empty());
    }
}

#[tokio::test]
async fn logo_needs_settings_manage() {
    for t in cores().await {
        let (core, _dir) = with_dir(&t);
        let reader = actor_with(&[]);
        let r = core.set_firm_logo(&reader, png_rgba(8, 8)).await;
        assert!(matches!(
            r,
            Err(CoreError::Forbidden(Permission::SettingsManage))
        ));
        let r = core.remove_firm_logo(&reader).await;
        assert!(matches!(
            r,
            Err(CoreError::Forbidden(Permission::SettingsManage))
        ));
        core.set_firm_logo(&admin(), png_rgba(8, 8)).await.unwrap();
        assert!(
            core.firm_logo(&reader).await.unwrap().is_some(),
            "{}",
            t.label
        );
        assert!(core.firm_logo_file(&reader).await.unwrap().is_some());
    }
}

#[tokio::test]
async fn logo_is_not_an_order_attachment() {
    for t in cores().await {
        let (core, dir) = with_dir(&t);
        let order = create_order_with(&t, |_| {}).await;
        core.set_firm_logo(&admin(), png_rgba(8, 8)).await.unwrap();
        let logo_id = logo_rows(&core).await[0];
        assert!(
            core.list_order_attachments(&admin(), order.id)
                .await
                .unwrap()
                .is_empty()
        );
        core.cleanup_drafts(Duration::zero()).await.unwrap();
        assert!(
            core.firm_logo(&admin()).await.unwrap().is_some(),
            "{}",
            t.label
        );
        assert_eq!(logo_files(dir.path()).len(), 1);

        // Обычные вложения не видят и не меняют логотип, даже с правом на заказы.
        let editor = actor_with(&[Permission::OrdersEdit, Permission::OrdersCreate]);
        let r = core.delete_attachment(&editor, logo_id).await;
        assert!(matches!(r, Err(CoreError::NotFound { .. })), "{}", t.label);
        let r = core
            .update_attachment_caption(&editor, logo_id, "x".into())
            .await;
        assert!(matches!(r, Err(CoreError::NotFound { .. })));
        let r = core
            .attachment_file(&editor, logo_id, AttachmentVariant::Full)
            .await;
        assert!(matches!(r, Err(CoreError::NotFound { .. })));
        assert!(core.firm_logo(&admin()).await.unwrap().is_some());
        assert_eq!(logo_files(dir.path()).len(), 1);
    }
}

#[tokio::test]
async fn remove_logo_is_idempotent() {
    for t in cores().await {
        let (core, dir) = with_dir(&t);
        core.remove_firm_logo(&admin()).await.unwrap();
        assert_eq!(count_audit(&core, "settings.logo_removed").await, 0);
        core.set_firm_logo(&admin(), png_rgba(8, 8)).await.unwrap();
        core.remove_firm_logo(&admin()).await.unwrap();
        core.remove_firm_logo(&admin()).await.unwrap();
        assert!(
            core.firm_logo(&admin()).await.unwrap().is_none(),
            "{}",
            t.label
        );
        assert!(core.firm_logo_file(&admin()).await.unwrap().is_none());
        assert!(logo_files(dir.path()).is_empty());
        assert_eq!(count_audit(&core, "settings.logo_removed").await, 1);
    }
}

#[tokio::test]
async fn logo_needs_data_dir() {
    for t in cores().await {
        let r = t.core.set_firm_logo(&admin(), png_rgba(8, 8)).await;
        assert!(matches!(r, Err(CoreError::Unavailable(_))), "{}", t.label);
        assert!(t.core.firm_logo_file(&admin()).await.is_err());
    }
}

/// Повреждённый на диске файл логотипа не ломает печать: документ выходит без логотипа.
#[tokio::test]
async fn broken_logo_file_prints_without_logo() {
    for t in cores().await {
        let (core, _dir) = with_dir(&t);
        let order = core
            .create_order(&admin(), sample_new_order())
            .await
            .unwrap();
        let actor = admin();
        let export = |output| {
            let req = DocumentRequest {
                kind: DocumentKind::IntakeAct,
                order_id: Some(order.id),
                sample: false,
                options: RenderOptions::default(),
                draft: None,
                pages: None,
            };
            core.export_document(&actor, req, output)
        };
        let pages = || async {
            match export(DocumentOutput::Png { dpi: 72 }).await.unwrap().bytes {
                DocumentBytes::Png(pages) => pages,
                other => panic!("{}: {other:?}", t.label),
            }
        };
        let without_logo = pages().await;
        core.set_firm_logo(&admin(), png_rgba(64, 32))
            .await
            .unwrap();
        // Подмена повреждения: подпись PNG на месте, дальше мусор.
        let id = logo_rows(&core).await[0];
        let file = core
            .data_dir_path()
            .unwrap()
            .join(format!("attachments/logo/{id}.png"));
        assert!(file.exists(), "{}: {}", t.label, file.display());
        std::fs::write(&file, b"\x89PNG\r\n\x1a\nnot a picture at all").unwrap();

        let act = export(DocumentOutput::Pdf).await;
        assert!(act.is_ok(), "{}: {act:?}", t.label);
        assert_eq!(
            pages().await,
            without_logo,
            "{}: лист не как без логотипа",
            t.label
        );
    }
}
