//! Файлы вложений на диске: пути только внутри `<папка данных>/attachments`.

use std::path::{Component, Path, PathBuf};

/// Длинная сторона миниатюры, пикселей.
pub(crate) const THUMB_SIDE: u32 = 320;
/// Качество JPEG миниатюры.
pub(crate) const THUMB_QUALITY: u8 = 80;

/// Путь внутри папки данных. Отказывает пустому, абсолютному, с `\`, `..` и не из `attachments/`.
pub fn resolve(data_dir: &Path, rel: &str) -> Option<PathBuf> {
    if rel.is_empty() || rel.contains('\\') {
        return None;
    }
    let path = Path::new(rel);
    let mut parts = path.components();
    if !matches!(parts.next(), Some(Component::Normal(first)) if first == "attachments") {
        return None;
    }
    let rest: Vec<_> = parts.collect();
    if rest.is_empty() || !rest.iter().all(|c| matches!(c, Component::Normal(_))) {
        return None;
    }
    Some(data_dir.join(path))
}

/// Имя файла миниатюры рядом с основным: `<uuid>.thumb.jpg`.
pub(crate) fn new_thumb_rel(main_rel: &str) -> String {
    let dir = main_rel.rsplit_once('/').map_or("", |(d, _)| d);
    format!("{dir}/{}.thumb.jpg", uuid::Uuid::new_v4())
}

/// Строит миниатюру JPEG (длинная сторона до 320 px) и пишет её в `to`.
pub(crate) fn write_thumbnail(from: &Path, to: &Path) -> Result<(), String> {
    let img = image::ImageReader::open(from)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .decode()
        .map_err(|e| e.to_string())?;
    let thumb = img.thumbnail(THUMB_SIDE, THUMB_SIDE).to_rgb8();
    let file = std::fs::File::create(to).map_err(|e| e.to_string())?;
    let mut out = std::io::BufWriter::new(file);
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, THUMB_QUALITY)
        .encode_image(&image::DynamicImage::ImageRgb8(thumb))
        .map_err(|e| e.to_string())?;
    std::io::Write::flush(&mut out).map_err(|e| e.to_string())
}

/// Удаляет файлы после фиксации записи: «нет файла» не ошибка, остальное — в журнал работы.
pub(crate) fn remove_files(paths: &[PathBuf]) {
    for path in paths {
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => tracing::warn!(path = %path.display(), error = %e, "файл вложения не удалён"),
        }
    }
}
