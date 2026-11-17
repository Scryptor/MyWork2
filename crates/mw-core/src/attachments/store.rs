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

/// Имя миниатюры, выводимое из основного пути: `<файл>.thumb.jpg` рядом с ним.
/// Одинаковое для всех запросов, поэтому параллельные запросы пишут один и тот же файл.
pub(crate) fn thumb_rel(main_rel: &str) -> String {
    format!("{main_rel}.thumb.jpg")
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
    // Сначала во временный файл, затем атомарно на место: читатель не увидит половину файла.
    let tmp = to.with_extension(format!("tmp-{}", uuid::Uuid::new_v4()));
    let written = (|| {
        let file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        let mut out = std::io::BufWriter::new(file);
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, THUMB_QUALITY)
            .encode_image(&image::DynamicImage::ImageRgb8(thumb))
            .map_err(|e| e.to_string())?;
        std::io::Write::flush(&mut out).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, to).map_err(|e| e.to_string())
    })();
    if written.is_err() {
        std::fs::remove_file(&tmp).ok();
    }
    written
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
