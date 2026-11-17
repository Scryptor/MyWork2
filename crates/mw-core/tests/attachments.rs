//! Вложения заказов: файлы старой программы, чтение, подпись, удаление.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use std::io::Cursor;
use std::path::Path;

use common::*;
use mw_core::{AttachmentKind, AttachmentVariant, Core, CoreError, ImportMode, ImportedLegacyRow};
use mw_domain::permission::Permission;
use serde_json::json;

fn jpeg_bytes(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbImage::from_pixel(w, h, image::Rgb([200, 30, 30]));
    let mut out = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut out, image::ImageFormat::Jpeg)
        .unwrap();
    out.into_inner()
}

fn put(dir: &Path, rel: &str, bytes: &[u8]) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn legacy_file(key: &str, order: i64, stored_as: &str, size: usize) -> ImportedLegacyRow {
    ImportedLegacyRow {
        source: "ClientFiles".into(),
        source_key: key.into(),
        target: Some(mw_core::LegacyTarget::Order(order)),
        raw: json!({ "stored_as": stored_as, "size": size, "sha256": "00" }),
    }
}

/// Core с папкой данных и импортом: у заказа 5 фото 1000x500 и видео.
async fn imported(core: Core, dir: &Path) -> Core {
    let core = core.with_attachments_dir(dir.to_path_buf());
    let photo = jpeg_bytes(1000, 500);
    put(dir, "attachments/orders/5/photo.jpg", &photo);
    put(dir, "attachments/orders/5/video.mp4", b"video");
    let mut bundle = sample_bundle();
    bundle.legacy_rows.push(legacy_file(
        "ClientFiles/1/5/video.mp4",
        5,
        "attachments/orders/5/video.mp4",
        5,
    ));
    bundle.legacy_rows.push(legacy_file(
        "ClientFiles/1/5/photo.jpg",
        5,
        "attachments/orders/5/photo.jpg",
        photo.len(),
    ));
    bundle.legacy_rows.push(ImportedLegacyRow {
        source: "ClientFiles".into(),
        source_key: "ClientFiles/1/5/broken".into(),
        target: Some(mw_core::LegacyTarget::Order(5)),
        raw: json!({ "something": "else" }),
    });
    core.import_bundle(&admin(), bundle, ImportMode::IntoEmpty)
        .await
        .unwrap();
    core
}

#[tokio::test]
async fn legacy_client_files_become_attachments() -> Result<(), CoreError> {
    for t in cores().await {
        let dir = tempfile::tempdir().unwrap();
        let core = imported(t.core.clone(), dir.path()).await;
        let list = core.list_order_attachments(&admin(), 5).await?;
        let got: Vec<_> = list
            .iter()
            .map(|a| (a.kind, a.name.as_str(), a.legacy, a.missing))
            .collect();
        assert_eq!(
            got,
            vec![
                (AttachmentKind::Photo, "photo.jpg", true, false),
                (AttachmentKind::File, "video.mp4", true, false)
            ],
            "{}",
            t.label
        );
        assert_eq!(list[1].content_type, "video/mp4");
        assert_eq!(list[1].size, 5);
        // Повторный запуск ничего не дублирует.
        let added = mw_db::backfill::legacy_client_files(core.db())
            .await
            .unwrap();
        assert_eq!(added, 0, "{}", t.label);
        assert_eq!(core.list_order_attachments(&admin(), 5).await?.len(), 2);
    }
    Ok(())
}

#[tokio::test]
async fn missing_file_reads_as_not_found() -> Result<(), CoreError> {
    for t in cores().await {
        let dir = tempfile::tempdir().unwrap();
        let core = imported(t.core.clone(), dir.path()).await;
        std::fs::remove_file(dir.path().join("attachments/orders/5/video.mp4")).unwrap();
        let list = core.list_order_attachments(&admin(), 5).await?;
        assert!(!list[0].missing && list[1].missing, "{}", t.label);
        let res = core
            .attachment_file(&admin(), list[1].id, AttachmentVariant::Full)
            .await;
        assert!(
            matches!(res, Err(CoreError::NotFound { .. })),
            "{}",
            t.label
        );
        // У обычного файла миниатюры нет.
        let res = core
            .attachment_file(&admin(), list[1].id, AttachmentVariant::Thumb)
            .await;
        assert!(
            matches!(res, Err(CoreError::NotFound { .. })),
            "{}",
            t.label
        );
    }
    Ok(())
}

#[tokio::test]
async fn without_storage_file_operations_are_unavailable() {
    for t in cores().await {
        let res = t.core.list_order_attachments(&admin(), 1).await;
        assert!(
            matches!(res, Err(CoreError::Unavailable(_))),
            "{}: {res:?}",
            t.label
        );
    }
}

#[tokio::test]
async fn legacy_photo_gets_a_thumbnail_on_first_request() -> Result<(), CoreError> {
    for t in cores().await {
        let dir = tempfile::tempdir().unwrap();
        let core = imported(t.core.clone(), dir.path()).await;
        let id = core.list_order_attachments(&admin(), 5).await?[0].id;
        let first = core
            .attachment_file(&admin(), id, AttachmentVariant::Thumb)
            .await?;
        assert_eq!(first.content_type, "image/jpeg");
        assert!(first.path.starts_with(dir.path().join("attachments")));
        let (w, h) = image::image_dimensions(&first.path).unwrap();
        assert_eq!((w, h), (320, 160), "{}", t.label);
        let mtime = std::fs::metadata(&first.path).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let second = core
            .attachment_file(&admin(), id, AttachmentVariant::Thumb)
            .await?;
        assert_eq!(first.path, second.path);
        assert_eq!(
            std::fs::metadata(&second.path).unwrap().modified().unwrap(),
            mtime
        );
        let full = core
            .attachment_file(&admin(), id, AttachmentVariant::Full)
            .await?;
        assert_eq!(
            (full.name.as_str(), full.content_type.as_str()),
            ("photo.jpg", "image/jpeg")
        );
    }
    Ok(())
}

#[tokio::test]
async fn concurrent_first_thumbnail_requests_leave_one_file() -> Result<(), CoreError> {
    for t in cores().await {
        let dir = tempfile::tempdir().unwrap();
        let core = imported(t.core.clone(), dir.path()).await;
        let id = core.list_order_attachments(&admin(), 5).await?[0].id;
        let who = admin();
        let (a, b) = tokio::join!(
            core.attachment_file(&who, id, AttachmentVariant::Thumb),
            core.attachment_file(&who, id, AttachmentVariant::Thumb)
        );
        let (a, b) = (a?, b?);
        assert_eq!(a.path, b.path, "{}", t.label);
        let files: Vec<_> = std::fs::read_dir(dir.path().join("attachments/orders/5"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(files.len(), 3, "{}: {files:?}", t.label); // фото, видео, одна миниатюра
        assert!(!files.iter().any(|f| f.contains("tmp-")), "{files:?}");
    }
    Ok(())
}

#[tokio::test]
async fn thumbnail_of_a_concurrently_deleted_attachment_is_not_found_and_leaves_no_file()
-> Result<(), CoreError> {
    for t in cores().await {
        let dir = tempfile::tempdir().unwrap();
        let core = imported(t.core.clone(), dir.path()).await;
        let id = core.list_order_attachments(&admin(), 5).await?[0].id;
        let who = admin();
        let (thumb, deleted) = tokio::join!(
            core.attachment_file(&who, id, AttachmentVariant::Thumb),
            core.delete_attachment(&who, id)
        );
        deleted?;
        assert!(
            matches!(thumb, Ok(_) | Err(CoreError::NotFound { .. })),
            "{}: {thumb:?}",
            t.label
        );
        let left: Vec<_> = std::fs::read_dir(dir.path().join("attachments/orders/5"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(left, vec!["video.mp4".to_owned()], "{}", t.label);
    }
    Ok(())
}

#[tokio::test]
async fn caption_is_edited_with_history() -> Result<(), CoreError> {
    for t in cores().await {
        let dir = tempfile::tempdir().unwrap();
        let core = imported(t.core.clone(), dir.path()).await;
        let id = core.list_order_attachments(&admin(), 5).await?[0].id;
        let a = core
            .update_attachment_caption(&admin(), id, "  Экран  ".into())
            .await?;
        assert_eq!(a.caption, "Экран", "{}", t.label);
        assert_eq!(last_audit(&core, 5).await.action, "attachment.caption");
        let denied = core
            .update_attachment_caption(&actor_with(&[]), id, "x".into())
            .await;
        assert!(matches!(
            denied,
            Err(CoreError::Forbidden(Permission::OrdersEdit))
        ));
        let missing = core
            .update_attachment_caption(&admin(), 9999, "x".into())
            .await;
        assert!(matches!(missing, Err(CoreError::NotFound { .. })));
    }
    Ok(())
}

#[tokio::test]
async fn delete_removes_files_after_commit_and_writes_history() -> Result<(), CoreError> {
    for t in cores().await {
        let dir = tempfile::tempdir().unwrap();
        let core = imported(t.core.clone(), dir.path()).await;
        let id = core.list_order_attachments(&admin(), 5).await?[0].id;
        let thumb = core
            .attachment_file(&admin(), id, AttachmentVariant::Thumb)
            .await?
            .path;
        let main = dir.path().join("attachments/orders/5/photo.jpg");
        assert!(main.exists() && thumb.exists());

        let denied = core.delete_attachment(&actor_with(&[]), id).await;
        assert!(matches!(
            denied,
            Err(CoreError::Forbidden(Permission::OrdersEdit))
        ));
        assert!(main.exists());

        core.delete_attachment(&admin(), id).await?;
        assert!(!main.exists() && !thumb.exists(), "{}", t.label);
        assert_eq!(core.list_order_attachments(&admin(), 5).await?.len(), 1);
        let last = last_audit(&core, 5).await;
        assert_eq!(last.action, "attachment.deleted");
        assert_eq!(
            last.details.unwrap(),
            json!({ "name": "photo.jpg", "path": "attachments/orders/5/photo.jpg" })
        );
        let again = core.delete_attachment(&admin(), id).await;
        assert!(matches!(again, Err(CoreError::NotFound { .. })));
    }
    Ok(())
}

#[test]
fn resolve_refuses_paths_outside_attachments() {
    let root = Path::new("/data");
    for bad in [
        "../x",
        "/etc/x",
        "attachments\\..\\x",
        "attachments/../x",
        "x/y",
        "",
    ] {
        assert!(mw_core::attachments::resolve(root, bad).is_none(), "{bad}");
    }
    assert_eq!(
        mw_core::attachments::resolve(root, "attachments/orders/1/a.jpg"),
        Some(root.join("attachments/orders/1/a.jpg"))
    );
}
