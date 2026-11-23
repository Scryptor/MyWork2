//! Логотип фирмы: одна картинка PNG для печатных документов.
//!
//! Это строка `attachments` вида `logo` без заказа и без черновика, поэтому очистка черновиков
//! её не трогает, а обычные вложения её не видят (см. `LOGO_KIND`). Файл лежит в
//! `attachments/logo/<номер>.png`. Новая загрузка заменяет прежнюю строку и файл.

use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;

use image::imageops::FilterType;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};
use mw_db::entity::attachment;
use mw_domain::permission::Permission;
use mw_domain::validation::FieldError;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::{Deserialize, Serialize};

use super::{AttachmentFile, LOGO_KIND, store};
use crate::{Actor, Core, CoreError, NewAuditEntry, write_audit};

/// Больше этого загрузку не читаем, байт.
const MAX_LOGO_BYTES: usize = 5 * 1024 * 1024;
/// Длинная сторона логотипа после обработки, пикселей.
const MAX_SIDE: u32 = 1200;
/// Больше этого по любой стороне картинку не читаем (защита от «бомбы»).
const MAX_DECODE_SIDE: u32 = 12_000;
/// Предел памяти при чтении картинки.
const MAX_DECODE_ALLOC: u64 = 256 * 1024 * 1024;
const LOGO_DIR: &str = "attachments/logo";

/// Логотип фирмы.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct FirmLogo {
    /// Ширина после обработки, пикселей.
    pub width: i32,
    /// Высота после обработки, пикселей.
    pub height: i32,
    /// Размер файла PNG, байт.
    pub size: i64,
}

impl From<&attachment::Model> for FirmLogo {
    fn from(m: &attachment::Model) -> Self {
        Self {
            width: m.width.unwrap_or(0),
            height: m.height.unwrap_or(0),
            size: m.size,
        }
    }
}

struct ProcessedLogo {
    png: Vec<u8>,
    width: u32,
    height: u32,
}

fn unsupported() -> CoreError {
    CoreError::Validation(vec![FieldError::new("logo", "unsupported")])
}

/// Приводит загрузку к PNG: формат — по содержимому (PNG, JPEG, WebP), поворот по EXIF,
/// длинная сторона не больше 1200 px, 8 бит на канал, прозрачность сохраняется, метаданных нет.
fn process(bytes: &[u8]) -> Result<ProcessedLogo, CoreError> {
    if bytes.len() > MAX_LOGO_BYTES {
        return Err(CoreError::Validation(vec![FieldError::new(
            "logo",
            "too_large",
        )]));
    }
    let format = image::guess_format(bytes).map_err(|_| unsupported())?;
    if !matches!(
        format,
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP
    ) {
        return Err(unsupported());
    }
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DECODE_SIDE);
    limits.max_image_height = Some(MAX_DECODE_SIDE);
    limits.max_alloc = Some(MAX_DECODE_ALLOC);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(|_| unsupported())?;
    let orientation = decoder.orientation().ok();
    let mut img = DynamicImage::from_decoder(decoder).map_err(|_| unsupported())?;
    if let Some(o) = orientation {
        img.apply_orientation(o);
    }
    if img.width().max(img.height()) > MAX_SIDE {
        img = img.resize(MAX_SIDE, MAX_SIDE, FilterType::Lanczos3);
    }
    let img = match (img.color().has_alpha(), img.color().has_color()) {
        (true, _) => DynamicImage::ImageRgba8(img.to_rgba8()),
        (false, true) => DynamicImage::ImageRgb8(img.to_rgb8()),
        (false, false) => DynamicImage::ImageLuma8(img.to_luma8()),
    };
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Png)
        .map_err(|_| unsupported())?;
    Ok(ProcessedLogo {
        png: out.into_inner(),
        width: img.width(),
        height: img.height(),
    })
}

impl Core {
    async fn logo_row(&self) -> Result<Option<attachment::Model>, CoreError> {
        Ok(attachment::Entity::find()
            .filter(attachment::Column::Kind.eq(LOGO_KIND))
            .order_by_desc(attachment::Column::Id)
            .one(&self.db)
            .await?)
    }

    /// Сведения о логотипе фирмы; `None` — логотипа нет (достаточно войти).
    pub async fn firm_logo(&self, _actor: &Actor) -> Result<Option<FirmLogo>, CoreError> {
        Ok(self.logo_row().await?.as_ref().map(FirmLogo::from))
    }

    /// Файл логотипа для отдачи; `None` — логотипа нет или файла нет на диске (достаточно войти).
    pub async fn firm_logo_file(
        &self,
        _actor: &Actor,
    ) -> Result<Option<AttachmentFile>, CoreError> {
        let data_dir = self.data_dir()?;
        let Some(m) = self.logo_row().await? else {
            return Ok(None);
        };
        Ok(store::resolve(data_dir, &m.path)
            .filter(|p| p.exists())
            .map(|path| AttachmentFile {
                path,
                content_type: m.content_type,
                name: m.name,
            }))
    }

    /// Байты PNG для печати документов; `None` — логотипа нет, файла нет или папка данных не задана.
    #[allow(dead_code, reason = "логотип в документы подключается отдельным шагом")]
    pub(crate) async fn firm_logo_png(&self) -> Option<Arc<[u8]>> {
        let data_dir = self.data_dir.as_deref()?;
        let m = match self.logo_row().await {
            Ok(m) => m?,
            Err(e) => {
                tracing::warn!(error = %e, "логотип не прочитан из базы");
                return None;
            }
        };
        let path = store::resolve(data_dir, &m.path)?;
        match tokio::fs::read(&path).await {
            Ok(bytes) => Some(Arc::from(bytes)),
            Err(e) => {
                tracing::warn!(error = %e, "файл логотипа не прочитан");
                None
            }
        }
    }

    /// Загружает логотип (право `settings_manage`); прежний логотип заменяется.
    pub async fn set_firm_logo(
        &self,
        actor: &Actor,
        bytes: Vec<u8>,
    ) -> Result<FirmLogo, CoreError> {
        actor.require(Permission::SettingsManage)?;
        let data_dir = self.data_dir()?.to_path_buf();
        let processed = tokio::task::spawn_blocking(move || process(&bytes))
            .await
            .map_err(|e| CoreError::Internal(e.to_string()))??;
        // Новый файл пишется до фиксации, старый убирается после: при сбое не остаётся ни
        // потерянного нового файла, ни логотипа без файла.
        let mut written: Option<PathBuf> = None;
        let done = async {
            let tx = self.begin_write().await?;
            let now = self.now();
            let old = attachment::Entity::find()
                .filter(attachment::Column::Kind.eq(LOGO_KIND))
                .all(&tx)
                .await?;
            // Новая строка добавляется раньше удаления старой: так номера не совпадают.
            let inserted = attachment::ActiveModel {
                id: sea_orm::NotSet,
                order_id: Set(None),
                draft_key: Set(None),
                kind: Set(LOGO_KIND.to_owned()),
                name: Set("logo.png".to_owned()),
                caption: Set(String::new()),
                content_type: Set("image/png".to_owned()),
                size: Set(i64::try_from(processed.png.len()).unwrap_or(i64::MAX)),
                width: Set(i32::try_from(processed.width).ok()),
                height: Set(i32::try_from(processed.height).ok()),
                path: Set(format!("{LOGO_DIR}/new.png")),
                thumb_path: Set(None),
                legacy: Set(false),
                created_at: Set(now),
            }
            .insert(&tx)
            .await?;
            let rel = format!("{LOGO_DIR}/{}.png", inserted.id);
            let target = store::resolve(&data_dir, &rel)
                .ok_or_else(|| CoreError::Internal("недопустимый путь логотипа".into()))?;
            if let Some(parent) = target.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| CoreError::Internal(format!("файл не записан: {e}")))?;
            }
            written = Some(target.clone());
            tokio::fs::write(&target, &processed.png)
                .await
                .map_err(|e| CoreError::Internal(format!("файл не записан: {e}")))?;
            let model = attachment::ActiveModel {
                id: Set(inserted.id),
                path: Set(rel),
                ..Default::default()
            }
            .update(&tx)
            .await?;
            if !old.is_empty() {
                attachment::Entity::delete_many()
                    .filter(attachment::Column::Id.is_in(old.iter().map(|m| m.id)))
                    .exec(&tx)
                    .await?;
            }
            write_audit(
                &tx,
                now,
                actor,
                NewAuditEntry {
                    action: "settings.logo_updated",
                    order_id: None,
                    client_id: None,
                    summary: "Загружен логотип фирмы".to_owned(),
                    details: Some(serde_json::json!({
                        "width": processed.width,
                        "height": processed.height,
                        "size": model.size,
                    })),
                },
            )
            .await?;
            tx.commit().await?;
            Ok::<_, CoreError>((model, old))
        }
        .await;
        match done {
            Ok((model, old)) => {
                remove_old(&data_dir, &old);
                Ok(FirmLogo::from(&model))
            }
            Err(e) => {
                if let Some(path) = written {
                    store::remove_files(&[path]);
                }
                Err(e)
            }
        }
    }

    /// Убирает логотип (право `settings_manage`). Без логотипа ничего не делает и не пишет в журнал.
    pub async fn remove_firm_logo(&self, actor: &Actor) -> Result<(), CoreError> {
        actor.require(Permission::SettingsManage)?;
        let data_dir = self.data_dir()?.to_path_buf();
        let tx = self.begin_write().await?;
        let old = attachment::Entity::find()
            .filter(attachment::Column::Kind.eq(LOGO_KIND))
            .all(&tx)
            .await?;
        if old.is_empty() {
            tx.rollback().await.ok();
            return Ok(());
        }
        attachment::Entity::delete_many()
            .filter(attachment::Column::Id.is_in(old.iter().map(|m| m.id)))
            .exec(&tx)
            .await?;
        write_audit(
            &tx,
            self.now(),
            actor,
            NewAuditEntry {
                action: "settings.logo_removed",
                order_id: None,
                client_id: None,
                summary: "Удалён логотип фирмы".to_owned(),
                details: None,
            },
        )
        .await?;
        tx.commit().await?;
        remove_old(&data_dir, &old);
        Ok(())
    }
}

/// Убирает файлы прежних строк после фиксации.
fn remove_old(data_dir: &std::path::Path, old: &[attachment::Model]) {
    let files: Vec<PathBuf> = old
        .iter()
        .filter_map(|m| store::resolve(data_dir, &m.path))
        .collect();
    store::remove_files(&files);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use std::sync::Arc;

    use image::ImageEncoder;

    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::FixedClock;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba([1, 2, 3, 255]));
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    /// Настоящий PNG 13000x13000 из нулей: сжимается до малого размера (далеко от 5 МиБ), но
    /// шире предела разбора, поэтому отказ даёт только `Limits`, а не размер и не битый файл.
    #[test]
    fn oversized_image_is_rejected_by_decode_limits() {
        let side = MAX_DECODE_SIDE + 1000;
        let zeros = vec![0u8; side as usize * side as usize];
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new_with_quality(
            &mut bytes,
            image::codecs::png::CompressionType::Fast,
            image::codecs::png::FilterType::NoFilter,
        )
        .write_image(&zeros, side, side, image::ExtendedColorType::L8)
        .unwrap();
        drop(zeros);
        assert!(bytes.len() < MAX_LOGO_BYTES, "{}", bytes.len());
        // Тот же файл без пределов читается целиком: значит, отказывают именно пределы.
        let unlimited = ImageReader::with_format(Cursor::new(&bytes), ImageFormat::Png)
            .decode()
            .unwrap();
        assert_eq!(unlimited.width(), side);
        drop(unlimited);
        let err = process(&bytes).err().unwrap();
        assert!(
            matches!(
                &err,
                CoreError::Validation(f) if *f == vec![FieldError::new("logo", "unsupported")]
            ),
            "{err:?}"
        );
    }

    #[tokio::test]
    async fn firm_logo_png_follows_the_stored_logo() {
        for (label, db) in mw_db::testing::all_backends().await {
            let clock = Arc::new(FixedClock::new(
                Utc.with_ymd_and_hms(2026, 11, 11, 9, 0, 0).unwrap(),
            ));
            let dir = tempfile::tempdir().unwrap();
            let core = Core::open(db, clock, "UTC")
                .await
                .unwrap()
                .with_attachments_dir(dir.path().to_path_buf());
            let admin = Actor::unrestricted("Тест");
            assert!(core.firm_logo_png().await.is_none(), "{label}");
            core.set_firm_logo(&admin, png(20, 10)).await.unwrap();
            let bytes = core.firm_logo_png().await.unwrap();
            let img = image::load_from_memory(&bytes).unwrap();
            assert_eq!((img.width(), img.height()), (20, 10), "{label}");
            core.remove_firm_logo(&admin).await.unwrap();
            assert!(core.firm_logo_png().await.is_none(), "{label}");
        }
    }
}
