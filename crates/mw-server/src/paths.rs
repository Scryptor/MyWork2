use crate::config::{DATA_DIR_NAME, ServerError};
use std::ffi::OsString;
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

/// Семейство ОС для выбора папки данных.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataDirOs {
    Windows,
    MacOs,
    Other,
}

impl DataDirOs {
    /// ОС, под которой собрана программа.
    pub fn current() -> DataDirOs {
        if cfg!(windows) {
            DataDirOs::Windows
        } else if cfg!(target_os = "macos") {
            DataDirOs::MacOs
        } else {
            DataDirOs::Other
        }
    }
}

/// Папка данных ОС по переменным окружения (без внешних зависимостей):
/// Windows — `%APPDATA%`; macOS — `$HOME/Library/Application Support`;
/// иначе `$XDG_DATA_HOME` (если задана и абсолютна), иначе `$HOME/.local/share`.
pub fn os_data_dir_for(os: DataDirOs, env: &dyn Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let non_empty = |name: &str| env(name).filter(|value| !value.is_empty());
    match os {
        DataDirOs::Windows => non_empty("APPDATA").map(PathBuf::from),
        DataDirOs::MacOs => non_empty("HOME").map(|home| {
            PathBuf::from(home)
                .join("Library")
                .join("Application Support")
        }),
        DataDirOs::Other => non_empty("XDG_DATA_HOME")
            .map(PathBuf::from)
            // XDG requires an absolute POSIX path (starts with `/`). `has_root` gives the
            // same answer on Linux and keeps this OS-parameterised function host-independent.
            .filter(|path| path.has_root())
            .or_else(|| {
                non_empty("HOME").map(|home| PathBuf::from(home).join(".local").join("share"))
            }),
    }
}

/// Папка данных текущей ОС; `None`, если нужные переменные окружения не заданы.
pub fn os_data_dir() -> Option<PathBuf> {
    os_data_dir_for(DataDirOs::current(), &|name| std::env::var_os(name))
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

    fn env_of(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<OsString> {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| OsString::from(*value))
        }
    }

    #[test]
    fn os_data_dir_per_os() {
        let win = env_of(&[("APPDATA", "/ad"), ("HOME", "/h")]);
        assert_eq!(
            os_data_dir_for(DataDirOs::Windows, &win),
            Some(PathBuf::from("/ad"))
        );
        assert_eq!(
            os_data_dir_for(DataDirOs::Windows, &env_of(&[("HOME", "/h")])),
            None
        );

        let mac = env_of(&[("HOME", "/Users/u"), ("XDG_DATA_HOME", "/x")]);
        assert_eq!(
            os_data_dir_for(DataDirOs::MacOs, &mac),
            Some(PathBuf::from("/Users/u/Library/Application Support"))
        );

        let abs = env_of(&[("XDG_DATA_HOME", "/x"), ("HOME", "/h")]);
        assert_eq!(
            os_data_dir_for(DataDirOs::Other, &abs),
            Some(PathBuf::from("/x"))
        );
        let rel = env_of(&[("XDG_DATA_HOME", "rel"), ("HOME", "/h")]);
        assert_eq!(
            os_data_dir_for(DataDirOs::Other, &rel),
            Some(PathBuf::from("/h/.local/share"))
        );
        let empty = env_of(&[("XDG_DATA_HOME", ""), ("HOME", "/h")]);
        assert_eq!(
            os_data_dir_for(DataDirOs::Other, &empty),
            Some(PathBuf::from("/h/.local/share"))
        );
        assert_eq!(os_data_dir_for(DataDirOs::Other, &env_of(&[])), None);
    }
}
