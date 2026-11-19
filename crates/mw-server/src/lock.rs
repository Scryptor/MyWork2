//! Один процесс на папку данных: файл-замок `<данные>/mywork.lock`, взятый на всё время работы. Его берут
//! программа для компьютера и команды `serve` и `import-legacy`; `create-admin` — нет (одна короткая запись в
//! базу), встроенный `Server` сам его не берёт (его берёт тот, кто сервер поднимает).
//! Замок держит ОС (`flock` / `LockFileEx`): он снимается и при аварийном завершении процесса.

use std::fs::{File, OpenOptions, TryLockError};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Имя файла-замка в папке данных.
pub const LOCK_FILE: &str = "mywork.lock";

/// Взятый замок папки данных; отпускается при удалении.
#[derive(Debug)]
pub struct DataDirLock {
    _file: File,
}

/// Почему замок не взят.
#[derive(Debug, thiserror::Error)]
pub enum LockError {
    /// Папку данных уже занимает другая копия программы (`pid` — если ОС дала прочитать файл-замок).
    #[error(
        "папку данных {} уже использует другая копия программы{}",
        .dir.display(),
        .pid.map(|pid| format!(" (процесс {pid})")).unwrap_or_default()
    )]
    Busy { dir: PathBuf, pid: Option<u32> },
    #[error("не удалось открыть файл-замок {}: {source}", .path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Берёт замок папки данных (создаёт её при первом запуске).
pub fn acquire(data_dir: &Path) -> Result<DataDirLock, LockError> {
    let path = data_dir.join(LOCK_FILE);
    let io_error = |source| LockError::Io {
        path: path.clone(),
        source,
    };
    std::fs::create_dir_all(data_dir).map_err(io_error)?;
    // Без `truncate`: файл занятой папки не трогается, пока замок не взят.
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(io_error)?;
    match file.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => {
            // На Windows занятый замком файл не читается: тогда номера процесса нет.
            let pid = std::fs::read_to_string(&path)
                .ok()
                .and_then(|text| text.trim().parse().ok());
            return Err(LockError::Busy {
                dir: data_dir.to_path_buf(),
                pid,
            });
        }
        Err(TryLockError::Error(source)) => return Err(io_error(source)),
    }
    // Номер процесса — для человека, который смотрит в папку; замку он не нужен.
    let pid = std::process::id().to_string();
    if let Err(e) = file
        .set_len(0)
        .and_then(|()| file.write_all(pid.as_bytes()))
    {
        tracing::warn!(
            "не удалось записать номер процесса в {}: {e}",
            path.display()
        );
    }
    Ok(DataDirLock { _file: file })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_lock_on_the_same_folder_fails_until_the_first_is_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("MyWork3");
        let first = acquire(&data).unwrap();
        assert!(data.join(LOCK_FILE).is_file());
        match acquire(&data) {
            Err(error @ LockError::Busy { .. }) => {
                let LockError::Busy { ref dir, pid } = error else {
                    unreachable!()
                };
                assert_eq!(dir, &data);
                if cfg!(unix) {
                    assert_eq!(pid, Some(std::process::id()));
                }
                let text = error.to_string();
                assert!(text.contains("уже использует другая копия"), "{text}");
            }
            other => panic!("второй замок взят: {other:?}"),
        }
        drop(first);
        let again = acquire(&data).unwrap();
        drop(again);
    }

    #[test]
    fn different_folders_do_not_block_each_other() {
        let dir = tempfile::tempdir().unwrap();
        let one = acquire(&dir.path().join("one")).unwrap();
        let two = acquire(&dir.path().join("two")).unwrap();
        drop((one, two));
    }

    #[test]
    fn the_lock_file_names_the_process_that_holds_it() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(LOCK_FILE),
            "старое содержимое, длиннее номера",
        )
        .unwrap();
        drop(acquire(dir.path()).unwrap());
        // Читается после снятия: на Windows занятый замком файл не читается и из другого дескриптора.
        let text = std::fs::read_to_string(dir.path().join(LOCK_FILE)).unwrap();
        assert_eq!(text, std::process::id().to_string());
    }
}
