//! Файлы отчёта об импорте в папке данных: общий код команды `import-legacy` и импорта из интерфейса.

use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;

use crate::LegacyError;
use crate::report::ImportReport;

/// Папка отчётов внутри папки данных MyWork 3.
pub const REPORTS_DIR: &str = "import-reports";

fn io_error(context: String) -> impl FnOnce(std::io::Error) -> LegacyError {
    move |source| LegacyError::Io { context, source }
}

/// Пишет `<data_dir>/import-reports/mywork2-<ГГГГММДД-ЧЧММСС>.txt` (`full_text`) и `.json`;
/// `at` — время по `Core::now()`. Возвращает пути текстового и JSON-отчёта.
pub fn write_report_files(
    data_dir: &Path,
    at: NaiveDateTime,
    report: &ImportReport,
) -> Result<(PathBuf, PathBuf), LegacyError> {
    let json =
        serde_json::to_string_pretty(&report.to_json()).map_err(|error| LegacyError::Io {
            context: "не удалось подготовить отчёт JSON".into(),
            source: std::io::Error::other(error),
        })?;
    let stamp = at.format("%Y%m%d-%H%M%S").to_string();
    write_named(
        &data_dir.join(REPORTS_DIR),
        &stamp,
        &report.full_text(),
        &json,
    )
}

/// Пишет `mywork2-<stamp>.txt/.json` в `dir`; занятое имя получает суффикс `-2`, `-3`…
fn write_named(
    dir: &Path,
    stamp: &str,
    text: &str,
    json: &str,
) -> Result<(PathBuf, PathBuf), LegacyError> {
    std::fs::create_dir_all(dir).map_err(io_error(format!(
        "не удалось создать папку {}",
        dir.display()
    )))?;
    let mut n = 1;
    loop {
        let base = if n == 1 {
            format!("mywork2-{stamp}")
        } else {
            format!("mywork2-{stamp}-{n}")
        };
        let text_path = dir.join(format!("{base}.txt"));
        let json_path = dir.join(format!("{base}.json"));
        if text_path.exists() || json_path.exists() {
            n += 1;
            continue;
        }
        write_new(&text_path, text)?;
        write_new(&json_path, json)?;
        return Ok((text_path, json_path));
    }
}

fn write_new(path: &Path, content: &str) -> Result<(), LegacyError> {
    use std::io::Write;
    let context = || format!("не удалось записать отчёт {}", path.display());
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(io_error(context()))?;
    file.write_all(content.as_bytes())
        .map_err(io_error(context()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_stamp_gets_numbered_names() {
        let dir = tempfile::tempdir().unwrap();
        let (t1, _) = write_named(dir.path(), "20260101-000000", "t", "{}").unwrap();
        let (t2, j2) = write_named(dir.path(), "20260101-000000", "t", "{}").unwrap();
        let (t3, _) = write_named(dir.path(), "20260101-000000", "t", "{}").unwrap();
        assert!(t1.ends_with("mywork2-20260101-000000.txt"));
        assert!(t2.ends_with("mywork2-20260101-000000-2.txt"));
        assert!(j2.ends_with("mywork2-20260101-000000-2.json"));
        assert!(t3.ends_with("mywork2-20260101-000000-3.txt"));
    }
}
