use crate::config::{DATA_DIR_NAME, ServerError};
use std::path::{Path, PathBuf};

/// Папка данных: явная; иначе `data` рядом с программой при наличии `portable.txt`;
/// иначе папка данных ОС + `MyWork3`.
pub fn resolve_data_dir(
    explicit: Option<PathBuf>,
    exe_dir: Option<&Path>,
    os_data_dir: Option<PathBuf>,
) -> Result<PathBuf, ServerError> {
    if let Some(dir) = explicit {
        return Ok(dir);
    }
    if let Some(exe_dir) = exe_dir
        && exe_dir.join("portable.txt").exists()
    {
        return Ok(exe_dir.join("data"));
    }
    os_data_dir
        .map(|dir| dir.join(DATA_DIR_NAME))
        .ok_or_else(|| {
            ServerError::Config(
                "не удалось определить папку данных: укажите --data-dir или MW_DATA_DIR".into(),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mw-paths-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn data_dir_explicit_then_portable_then_os() {
        let exe = temp_dir("exe");
        let _ = std::fs::remove_file(exe.join("portable.txt"));
        assert_eq!(
            resolve_data_dir(Some("/x".into()), Some(&exe), Some("/os".into())).unwrap(),
            PathBuf::from("/x")
        );
        assert_eq!(
            resolve_data_dir(None, Some(&exe), Some("/os".into())).unwrap(),
            PathBuf::from("/os").join("MyWork3")
        );
        std::fs::write(exe.join("portable.txt"), "").unwrap();
        assert_eq!(
            resolve_data_dir(None, Some(&exe), Some("/os".into())).unwrap(),
            exe.join("data")
        );
        assert!(resolve_data_dir(None, None, None).is_err());
        let _ = std::fs::remove_dir_all(&exe);
    }
}
