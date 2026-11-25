//! Настройки копий: файл `backup-settings.json` в папке данных и проверка папки копий.

use std::path::Path;

use mw_domain::validation::FieldError;

use super::{BackupSettings, KEEP_MAX, SETTINGS_FILE};

/// Читает настройки; нет файла или он испорчен — умолчания.
pub(crate) fn read(data_dir: &Path) -> BackupSettings {
    let mut settings: BackupSettings = std::fs::read(data_dir.join(SETTINGS_FILE))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    settings.keep = settings.keep.clamp(1, KEEP_MAX);
    settings
}

/// Пишет настройки во временный файл рядом и переименовывает его.
pub(crate) fn write(data_dir: &Path, settings: &BackupSettings) -> std::io::Result<()> {
    let tmp = data_dir.join(format!("{SETTINGS_FILE}.tmp"));
    let bytes = serde_json::to_vec_pretty(settings).map_err(std::io::Error::other)?;
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, data_dir.join(SETTINGS_FILE)).inspect_err(|_| {
        std::fs::remove_file(&tmp).ok();
    })
}

/// Папка копий: полный путь к существующей папке вне папки данных, в которую можно писать.
pub(crate) fn check_folder(data_dir: &Path, folder: &str) -> Result<(), FieldError> {
    let path = Path::new(folder);
    if !path.is_absolute() {
        return Err(FieldError::new("folder", "folder_relative"));
    }
    if !path.is_dir() {
        return Err(FieldError::new("folder", "folder_missing"));
    }
    let inside = match (std::fs::canonicalize(path), std::fs::canonicalize(data_dir)) {
        (Ok(folder), Ok(data)) => folder.starts_with(data),
        _ => path.starts_with(data_dir),
    };
    if inside {
        return Err(FieldError::new("folder", "folder_inside_data"));
    }
    let probe = path.join(".mywork-write-test");
    match std::fs::File::create(&probe) {
        Ok(_) => {
            std::fs::remove_file(&probe).ok();
            Ok(())
        }
        Err(_) => Err(FieldError::new("folder", "folder_read_only")),
    }
}
