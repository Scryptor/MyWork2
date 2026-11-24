//! Фото позиций склада: до трёх штук, права, журнал, отдельность от заказов и черновиков.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use common::{actor_with, admin, assert_validation, audit_details, cores};
use mw_core::{AttachmentVariant, CoreError, NewStockItem, StockItem, attachments};
use mw_domain::permission::Permission;
use mw_domain::stock::MAX_STOCK_PHOTOS;

fn png() -> Vec<u8> {
    let img = image::RgbImage::from_pixel(40, 30, image::Rgb([200, 30, 30]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

async fn item(core: &mw_core::Core) -> StockItem {
    core.create_stock_item(
        &admin(),
        NewStockItem {
            name: "Дисплей".into(),
            ..Default::default()
        },
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn three_photos_then_refusal() -> Result<(), CoreError> {
    for t in cores().await {
        let dir = tempfile::tempdir().unwrap();
        let core = t
            .core
            .clone()
            .with_attachments_dir(dir.path().to_path_buf());
        let it = item(&core).await;
        let mut ids = Vec::new();
        for n in 0..MAX_STOCK_PHOTOS {
            let a = core
                .add_stock_photo(&admin(), it.id, &format!("p{n}.png"), png())
                .await?;
            assert_eq!(a.stock_item_id, Some(it.id), "{}", t.label);
            assert_eq!(a.order_id, None);
            ids.push(a.id);
        }
        let list = core.list_stock_photos(&admin(), it.id).await?;
        assert_eq!(list.iter().map(|a| a.id).collect::<Vec<_>>(), ids);
        let file = core
            .attachment_file(&admin(), ids[0], AttachmentVariant::Full)
            .await?;
        assert!(
            file.path
                .starts_with(dir.path().join(format!("attachments/stock/{}", it.id)))
        );
        assert!(
            attachments::resolve(dir.path(), &format!("attachments/stock/{}/x.jpg", it.id))
                .is_some()
        );
        let res = core.add_stock_photo(&admin(), it.id, "p4.png", png()).await;
        assert_validation(res, &[("file", "too_many")]);
        match core.add_stock_photo(&admin(), it.id, "p4.png", png()).await {
            Err(CoreError::Validation(f)) => {
                assert_eq!(f[0].message, Some("У позиции уже 3 фото"));
            }
            other => panic!("{other:?}"),
        }
        let dir_files = std::fs::read_dir(dir.path().join(format!("attachments/stock/{}", it.id)))
            .unwrap()
            .count();
        // Три фото и три миниатюры; отвергнутые не оставили файлов.
        assert_eq!(dir_files, 6, "{}", t.label);
        let added = audit_details(&core, "attachment.added").await;
        assert_eq!(added.len(), 3);
        // Журнал позиции видит добавление.
        let hist = core
            .stock_item_history(
                &admin(),
                it.id,
                mw_core::PageRequest {
                    limit: 50,
                    offset: 0,
                },
            )
            .await?;
        assert_eq!(
            hist.items
                .iter()
                .filter(|e| e.action == "attachment.added" && e.stock_item_id == Some(it.id))
                .count(),
            3
        );
    }
    Ok(())
}

#[tokio::test]
async fn simultaneous_uploads_never_leave_four() -> Result<(), CoreError> {
    for t in cores().await {
        let dir = tempfile::tempdir().unwrap();
        let core = t
            .core
            .clone()
            .with_attachments_dir(dir.path().to_path_buf());
        let it = item(&core).await;
        let mut tasks = Vec::new();
        for n in 0..6 {
            let core = core.clone();
            tasks.push(tokio::spawn(async move {
                core.add_stock_photo(&admin(), it.id, &format!("p{n}.png"), png())
                    .await
            }));
        }
        let mut ok = 0;
        for task in tasks {
            if task.await.unwrap().is_ok() {
                ok += 1;
            }
        }
        assert_eq!(ok, 3, "{}", t.label);
        assert_eq!(core.list_stock_photos(&admin(), it.id).await?.len(), 3);
        let files = std::fs::read_dir(dir.path().join(format!("attachments/stock/{}", it.id)))
            .unwrap()
            .count();
        assert_eq!(files, 6, "{}: лишние файлы отвергнутых", t.label);
    }
    Ok(())
}

#[tokio::test]
async fn not_a_photo_is_refused() -> Result<(), CoreError> {
    for t in cores().await {
        let dir = tempfile::tempdir().unwrap();
        let core = t
            .core
            .clone()
            .with_attachments_dir(dir.path().to_path_buf());
        let it = item(&core).await;
        let res = core
            .add_stock_photo(&admin(), it.id, "a.pdf", b"%PDF-1.4 text".to_vec())
            .await;
        match res {
            Err(CoreError::Validation(f)) => {
                assert_eq!(
                    (f[0].field.as_str(), f[0].code.as_str(), f[0].message),
                    (
                        "file",
                        "not_a_photo",
                        Some("У позиции склада — только фото")
                    ),
                    "{}",
                    t.label
                );
            }
            other => panic!("{other:?}"),
        }
        assert!(core.list_stock_photos(&admin(), it.id).await?.is_empty());
        let stock_dir = dir.path().join(format!("attachments/stock/{}", it.id));
        let left = std::fs::read_dir(stock_dir).map_or(0, Iterator::count);
        assert_eq!(left, 0, "{}", t.label);
    }
    Ok(())
}

#[tokio::test]
async fn rights_follow_stock() -> Result<(), CoreError> {
    for t in cores().await {
        let dir = tempfile::tempdir().unwrap();
        let core = t
            .core
            .clone()
            .with_attachments_dir(dir.path().to_path_buf());
        let it = item(&core).await;
        let photo = core
            .add_stock_photo(&admin(), it.id, "a.png", png())
            .await?;
        let viewer = actor_with(&[Permission::StockView]);
        let editor = actor_with(&[Permission::StockEdit]);
        // Заказные права ничего не дают.
        let orders = actor_with(&[Permission::OrdersEdit, Permission::OrdersCreate]);
        for who in [&viewer, &orders] {
            assert!(matches!(
                core.add_stock_photo(who, it.id, "b.png", png()).await,
                Err(CoreError::Forbidden(Permission::StockEdit))
            ));
            assert!(matches!(
                core.update_attachment_caption(who, photo.id, "x".into())
                    .await,
                Err(CoreError::Forbidden(Permission::StockEdit))
            ));
            assert!(matches!(
                core.delete_attachment(who, photo.id).await,
                Err(CoreError::Forbidden(Permission::StockEdit))
            ));
        }
        // Чтение — только с `stock_view`.
        assert!(matches!(
            core.list_stock_photos(&editor, it.id).await,
            Err(CoreError::Forbidden(Permission::StockView))
        ));
        for v in [AttachmentVariant::Full, AttachmentVariant::Thumb] {
            assert!(matches!(
                core.attachment_file(&editor, photo.id, v).await,
                Err(CoreError::Forbidden(Permission::StockView))
            ));
            assert!(matches!(
                core.attachment_file(&orders, photo.id, v).await,
                Err(CoreError::Forbidden(Permission::StockView))
            ));
            core.attachment_file(&viewer, photo.id, v).await?;
        }
        assert_eq!(core.list_stock_photos(&viewer, it.id).await?.len(), 1);
        // Правка: подпись и удаление с журналом позиции.
        let cap = core
            .update_attachment_caption(&editor, photo.id, " вид сзади ".into())
            .await?;
        assert_eq!(
            (cap.caption.as_str(), cap.stock_item_id),
            ("вид сзади", Some(it.id))
        );
        core.delete_attachment(&editor, photo.id).await?;
        assert!(core.list_stock_photos(&viewer, it.id).await?.is_empty());
        let dels = audit_details(&core, "attachment.deleted").await;
        assert_eq!(dels.len(), 1, "{}", t.label);
        let hist = core
            .stock_item_history(
                &viewer,
                it.id,
                mw_core::PageRequest {
                    limit: 50,
                    offset: 0,
                },
            )
            .await?;
        let actions: Vec<_> = hist.items.iter().map(|e| e.action.as_str()).collect();
        assert!(actions.contains(&"attachment.deleted"), "{actions:?}");
        assert!(actions.contains(&"attachment.caption"), "{actions:?}");
    }
    Ok(())
}

#[tokio::test]
async fn stock_photos_stay_out_of_orders_and_drafts() -> Result<(), CoreError> {
    for t in cores().await {
        let dir = tempfile::tempdir().unwrap();
        let core = t
            .core
            .clone()
            .with_attachments_dir(dir.path().to_path_buf());
        let it = item(&core).await;
        core.add_stock_photo(&admin(), it.id, "a.png", png())
            .await?;
        // Номер позиции совпадает с номером заказа и ключ черновика не подходит: фото чужих
        // владельцев не видны.
        assert!(
            core.list_order_attachments(&admin(), it.id)
                .await?
                .is_empty()
        );
        assert!(
            core.list_draft_attachments(&admin(), "stock")
                .await?
                .is_empty()
        );
    }
    Ok(())
}

#[tokio::test]
async fn deleted_item_takes_no_photos() -> Result<(), CoreError> {
    for t in cores().await {
        let dir = tempfile::tempdir().unwrap();
        let core = t
            .core
            .clone()
            .with_attachments_dir(dir.path().to_path_buf());
        let it = item(&core).await;
        let photo = core
            .add_stock_photo(&admin(), it.id, "a.png", png())
            .await?;
        core.delete_stock_item(&admin(), it.id, it.version).await?;
        assert_validation(
            core.add_stock_photo(&admin(), it.id, "b.png", png()).await,
            &[("id", "deleted")],
        );
        // Мягкое удаление файлов не трогает.
        let file = core
            .attachment_file(&admin(), photo.id, AttachmentVariant::Full)
            .await?;
        assert!(file.path.exists(), "{}", t.label);
        // Нет такой позиции вовсе.
        assert!(matches!(
            core.add_stock_photo(&admin(), 9999, "b.png", png()).await,
            Err(CoreError::NotFound { .. })
        ));
    }
    Ok(())
}
