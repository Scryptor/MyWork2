//! Обработка фото: поворот по EXIF, удаление метаданных, уменьшение, миниатюра.

use std::io::Cursor;

use image::imageops::FilterType;
use image::{DynamicImage, ImageDecoder, ImageError, ImageFormat, ImageReader, Limits};

use super::store::{THUMB_QUALITY, THUMB_SIDE};

/// Длинная сторона фото после обработки, пикселей.
const MAX_SIDE: u32 = 2560;
/// Качество JPEG фото.
const PHOTO_QUALITY: u8 = 85;
/// Больше этого по любой стороне картинку не читаем.
const MAX_DECODE_SIDE: u32 = 12_000;
/// Предел памяти при чтении картинки.
const MAX_DECODE_ALLOC: u64 = 512 * 1024 * 1024;

/// Готовое фото: основной файл и миниатюра.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessedPhoto {
    pub jpeg: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub thumb: Vec<u8>,
}

/// Форматы, которые читаются и становятся фото (собраны только их декодеры).
const PHOTO_FORMATS: [ImageFormat; 5] = [
    ImageFormat::Jpeg,
    ImageFormat::Png,
    ImageFormat::WebP,
    ImageFormat::Gif,
    ImageFormat::Bmp,
];

/// Почему фото не обработано.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhotoError {
    /// Формат узнан, но фото из него не делается (`format` — его имя, `heic` для HEIC),
    /// или это не картинка (`format` пуст).
    Unsupported { format: Option<String> },
    /// Картинка узнана, но не читается.
    Broken,
    /// Картинка слишком большая.
    TooLarge,
}

/// Сигнатуры HEIC: основной бренд `ftyp` (байты 8..12) из семейства HEIC. Общие бренды HEIF
/// `mif1`/`msf1` не в счёт сами по себе — их ставит и AVIF; но с совместимым брендом
/// `heic`/`heix` (так пишут некоторые камеры) это HEIC.
pub(crate) fn is_heic(bytes: &[u8]) -> bool {
    const MAJOR: [&[u8]; 5] = [b"heic", b"heix", b"hevc", b"heim", b"heis"];
    const COMPATIBLE: [&[u8]; 2] = [b"heic", b"heix"];
    if bytes.get(4..8) != Some(b"ftyp") {
        return false;
    }
    let Some(major) = bytes.get(8..12) else {
        return false;
    };
    if MAJOR.contains(&major) {
        return true;
    }
    if major != b"mif1" && major != b"msf1" {
        return false;
    }
    // Совместимые бренды: с байта 16 до конца блока `ftyp` (но не дальше начала данных).
    let size = bytes
        .get(0..4)
        .and_then(|b| <[u8; 4]>::try_from(b).ok())
        .map_or(0, |b| u32::from_be_bytes(b) as usize);
    let end = size.clamp(16, 256).min(bytes.len());
    bytes
        .get(16..end)
        .is_some_and(|brands| brands.chunks_exact(4).any(|b| COMPATIBLE.contains(&b)))
}

fn encode_jpeg(img: &DynamicImage, quality: u8) -> Result<Vec<u8>, PhotoError> {
    let rgb = img.to_rgb8();
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality)
        .encode_image(&rgb)
        .map_err(|_| PhotoError::Broken)?;
    Ok(out)
}

/// Накладывает прозрачность на белый фон.
fn flatten(img: DynamicImage) -> DynamicImage {
    if !img.color().has_alpha() {
        return img;
    }
    let mut rgba = img.to_rgba8();
    for p in rgba.pixels_mut() {
        let a = u32::from(p.0[3]);
        for c in &mut p.0[..3] {
            let v = (u32::from(*c) * a + 255 * (255 - a)) / 255;
            *c = u8::try_from(v).unwrap_or(255);
        }
        p.0[3] = 255;
    }
    DynamicImage::ImageRgba8(rgba)
}

fn map_error(e: &ImageError) -> PhotoError {
    match e {
        ImageError::Limits(_) => PhotoError::TooLarge,
        _ => PhotoError::Broken,
    }
}

/// Приводит картинку к JPEG: формат по сигнатуре, поворот по EXIF, белый фон под прозрачностью,
/// длинная сторона не больше 2560 px, метаданных в выходе нет; плюс миниатюра 320 px.
pub fn process_photo(bytes: &[u8]) -> Result<ProcessedPhoto, PhotoError> {
    if is_heic(bytes) {
        return Err(PhotoError::Unsupported {
            format: Some("heic".into()),
        });
    }
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| PhotoError::Broken)?;
    match reader.format() {
        None => return Err(PhotoError::Unsupported { format: None }),
        Some(f) if !PHOTO_FORMATS.contains(&f) => {
            return Err(PhotoError::Unsupported {
                format: Some(format!("{f:?}").to_lowercase()),
            });
        }
        Some(_) => {}
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DECODE_SIDE);
    limits.max_image_height = Some(MAX_DECODE_SIDE);
    limits.max_alloc = Some(MAX_DECODE_ALLOC);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(|e| map_error(&e))?;
    let orientation = decoder.orientation().ok();
    let mut img = DynamicImage::from_decoder(decoder).map_err(|e| map_error(&e))?;
    if let Some(o) = orientation {
        img.apply_orientation(o);
    }
    let mut img = flatten(img);
    if img.width().max(img.height()) > MAX_SIDE {
        img = img.resize(MAX_SIDE, MAX_SIDE, FilterType::Triangle);
    }
    let jpeg = encode_jpeg(&img, PHOTO_QUALITY)?;
    let thumb_img = if img.width().max(img.height()) > THUMB_SIDE {
        img.resize(THUMB_SIDE, THUMB_SIDE, FilterType::Triangle)
    } else {
        img.clone()
    };
    Ok(ProcessedPhoto {
        jpeg,
        width: img.width(),
        height: img.height(),
        thumb: encode_jpeg(&thumb_img, THUMB_QUALITY)?,
    })
}

/// Размер вложения не больше, байт.
pub const MAX_ATTACHMENT_BYTES: usize = 25 * 1024 * 1024;

const IMAGE_EXTENSIONS: [&str; 6] = ["jpg", "jpeg", "png", "webp", "gif", "bmp"];

/// Вложение, готовое к записи на диск.
pub(crate) struct Prepared {
    pub photo: bool,
    pub name: String,
    pub ext: String,
    pub content_type: String,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub main: Vec<u8>,
    pub thumb: Option<Vec<u8>>,
}

fn upload_error(code: &'static str, text: &'static str) -> crate::CoreError {
    crate::CoreError::Validation(vec![mw_domain::validation::FieldError::with_message(
        "file", code, text,
    )])
}

fn clean_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("").trim();
    if base.is_empty() {
        "файл".to_owned()
    } else {
        base.chars().take(200).collect()
    }
}

/// Расширение из имени: строчные, 1–8 латинских букв и цифр; иначе пусто.
fn extension_of(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(_, e)| e.to_lowercase())
        .filter(|e| (1..=8).contains(&e.len()) && e.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or_default()
}

/// Имя обработанного фото: то же, но с расширением `.jpg` (файл теперь JPEG).
fn jpeg_name(name: &str) -> String {
    let stem = match name.rsplit_once('.') {
        Some((stem, _)) if !extension_of(name).is_empty() && !stem.is_empty() => stem,
        _ => name,
    };
    let stem: String = stem.chars().take(196).collect();
    format!("{stem}.jpg")
}

/// Разбирает загруженные байты: картинка одного из пяти форматов становится фото, остальное —
/// файлом как есть. Отказ — HEIC, а под именем картинки — нечитаемые или слишком большие байты.
pub(crate) fn prepare_upload(name: &str, bytes: Vec<u8>) -> Result<Prepared, crate::CoreError> {
    if bytes.len() > MAX_ATTACHMENT_BYTES {
        return Err(upload_error("too_large", "Файл больше 25 МБ"));
    }
    let name = clean_name(name);
    let ext = extension_of(&name);
    match process_photo(&bytes) {
        Ok(p) => Ok(Prepared {
            photo: true,
            name: jpeg_name(&name),
            ext: "jpg".into(),
            content_type: "image/jpeg".into(),
            width: i32::try_from(p.width).ok(),
            height: i32::try_from(p.height).ok(),
            main: p.jpeg,
            thumb: Some(p.thumb),
        }),
        Err(PhotoError::Unsupported { format: Some(f) }) if f == "heic" => Err(upload_error(
            "unsupported",
            "Формат HEIC не поддерживается: сохраните фото как JPEG",
        )),
        Err(e) if IMAGE_EXTENSIONS.contains(&ext.as_str()) => Err(match e {
            PhotoError::TooLarge => upload_error("too_large", "Картинка слишком большая"),
            _ => upload_error("broken", "Не удалось прочитать картинку"),
        }),
        // Не картинка, картинка другого формата (TIFF, AVIF…) или байты, лишь похожие на картинку
        // («BM…» в тексте): файл как есть.
        Err(_) => {
            let content_type = mime_guess::from_path(&name)
                .first()
                .map_or_else(|| "application/octet-stream".to_owned(), |m| m.to_string());
            Ok(Prepared {
                photo: false,
                name,
                ext,
                content_type,
                width: None,
                height: None,
                main: bytes,
                thumb: None,
            })
        }
    }
}
