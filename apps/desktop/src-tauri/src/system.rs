//! Команды программы для компьютера: перезапуск и открытие папки в файловом менеджере.
//!
//! Ошибки — строки по-русски: интерфейс показывает их в уведомлении как есть.

use std::path::PathBuf;
use tauri::AppHandle;

/// Перезапускает программу после просьбы о восстановлении из копии. Именно `request_restart`: перезапуск идёт через
/// `RunEvent::Exit`, и `shut_down` штатно останавливает сервер и отпускает замок папки данных до запуска новой
/// копии (`restart` на главном потоке события выхода пропускает).
#[tauri::command]
pub fn restart_app(app: AppHandle) {
    app.request_restart();
}

/// Открывает существующую папку в файловом менеджере системы. Путь в журнал не пишется.
#[tauri::command]
pub fn open_folder(path: String) -> Result<(), String> {
    let folder = existing_folder(&path)?;
    open::that_detached(folder).map_err(|e| format!("Не удалось открыть папку: {e}"))
}

/// Путь как передан, если это полный путь к существующей папке, иначе «Папки нет: <путь>». Без
/// `canonicalize`: на Windows он даёт `\\?\C:\…`, а такой путь Проводник не открывает.
pub(crate) fn existing_folder(path: &str) -> Result<PathBuf, String> {
    let folder = std::path::Path::new(path);
    if folder.is_absolute() && folder.is_dir() {
        Ok(folder.to_path_buf())
    } else {
        Err(format!("Папки нет: {path}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn existing_folder_accepts_only_a_folder() {
        let dir = tempfile::tempdir().unwrap();
        // Путь открывается как передан: канонический путь Windows (`\\?\C:\…`) Проводник не принимает.
        let ok = existing_folder(dir.path().to_str().unwrap()).unwrap();
        assert_eq!(ok, dir.path());
        let inner = dir.path().join("inner");
        std::fs::create_dir(&inner).unwrap();
        let dotted = dir.path().join("inner").join("..").join("inner");
        assert_eq!(existing_folder(dotted.to_str().unwrap()).unwrap(), dotted);

        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"x").unwrap();
        let path = file.to_str().unwrap();
        let err = existing_folder(path).unwrap_err();
        assert_eq!(err, format!("Папки нет: {path}"));

        assert!(
            existing_folder("relative/dir")
                .unwrap_err()
                .starts_with("Папки нет")
        );
        let missing = dir.path().join("missing");
        assert!(
            existing_folder(missing.to_str().unwrap())
                .unwrap_err()
                .starts_with("Папки нет")
        );
    }
}
