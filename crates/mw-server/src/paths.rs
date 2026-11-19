use crate::config::{DATA_DIR_NAME, ServerError};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Файл рядом с программой, переводящий её в портативный режим (данные — в `data` рядом).
pub const PORTABLE_FILE: &str = "portable.txt";

/// Имя папки данных в папке ОС: `MyWork3`, у отладочной сборки (`dev`) — `MyWork3-dev`. Сборка из рабочей
/// копии может применить ещё не выпущенную миграцию, и установленная программа на такой базе больше бы не
/// запустилась. Общее для сервера (`cargo run -p mw-server`) и программы для компьютера.
pub fn data_dir_name(dev: bool) -> &'static str {
    if dev { "MyWork3-dev" } else { DATA_DIR_NAME }
}

/// Портативная папка данных: `data` рядом с программой, если рядом лежит `portable.txt`. Внутри пакета
/// macOS (`*.app/Contents/MacOS`) файл не действует: данные внутри пакета пропали бы при его замене.
pub fn portable_data_dir(exe_dir: &Path) -> Option<PathBuf> {
    (!inside_macos_bundle(exe_dir) && exe_dir.join(PORTABLE_FILE).exists())
        .then(|| exe_dir.join("data"))
}

/// Папка `*.app/Contents/MacOS` (по имени, на любой ОС: так проверка не зависит от машины тестов).
fn inside_macos_bundle(exe_dir: &Path) -> bool {
    let name = |path: Option<&Path>| {
        path.and_then(Path::file_name)
            .map(|name| name.to_string_lossy().to_lowercase())
    };
    let contents = exe_dir.parent();
    let bundle = contents.and_then(Path::parent);
    name(Some(exe_dir)).as_deref() == Some("macos")
        && name(contents).as_deref() == Some("contents")
        && name(bundle).is_some_and(|name| name.ends_with(".app"))
}

/// Папка данных: явная; иначе `data` рядом с программой при наличии `portable.txt` (`portable_data_dir`);
/// иначе папка данных ОС + `data_dir_name(dev)`.
pub fn resolve_data_dir(
    explicit: Option<PathBuf>,
    exe_dir: Option<&Path>,
    os_data_dir: Option<PathBuf>,
    dev: bool,
) -> Result<PathBuf, ServerError> {
    if let Some(dir) = explicit {
        return Ok(dir);
    }
    if let Some(portable) = exe_dir.and_then(portable_data_dir) {
        return Ok(portable);
    }
    os_data_dir
        .map(|dir| dir.join(data_dir_name(dev)))
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
            resolve_data_dir(Some("/x".into()), Some(&exe), Some("/os".into()), false).unwrap(),
            PathBuf::from("/x")
        );
        assert_eq!(
            resolve_data_dir(None, Some(&exe), Some("/os".into()), false).unwrap(),
            PathBuf::from("/os").join("MyWork3")
        );
        std::fs::write(exe.join("portable.txt"), "").unwrap();
        for dev in [false, true] {
            assert_eq!(
                resolve_data_dir(None, Some(&exe), Some("/os".into()), dev).unwrap(),
                exe.join("data")
            );
        }
        assert!(resolve_data_dir(None, None, None, false).is_err());
        let _ = std::fs::remove_dir_all(&exe);
    }

    #[test]
    fn a_debug_build_keeps_its_data_apart_from_the_installed_program() {
        assert_eq!(data_dir_name(false), "MyWork3");
        assert_eq!(data_dir_name(true), "MyWork3-dev");
        let exe = temp_dir("dev");
        assert_eq!(
            resolve_data_dir(None, Some(&exe), Some("/os".into()), true).unwrap(),
            PathBuf::from("/os").join("MyWork3-dev")
        );
        assert_eq!(
            resolve_data_dir(Some("/x".into()), Some(&exe), Some("/os".into()), true).unwrap(),
            PathBuf::from("/x")
        );
        let _ = std::fs::remove_dir_all(&exe);
    }

    #[test]
    fn portable_txt_inside_a_macos_bundle_is_ignored() {
        let root = tempfile::tempdir().unwrap();
        let inside = root
            .path()
            .join("MyWork.app")
            .join("Contents")
            .join("MacOS");
        std::fs::create_dir_all(&inside).unwrap();
        std::fs::write(inside.join(PORTABLE_FILE), "").unwrap();
        assert_eq!(portable_data_dir(&inside), None);
        assert_eq!(
            resolve_data_dir(None, Some(&inside), Some("/os".into()), false).unwrap(),
            PathBuf::from("/os").join("MyWork3")
        );
        // Регистр имени пакета не важен; папка рядом с пакетом — обычная портативная.
        let upper = root.path().join("MW.APP").join("Contents").join("MacOS");
        std::fs::create_dir_all(&upper).unwrap();
        std::fs::write(upper.join(PORTABLE_FILE), "").unwrap();
        assert_eq!(portable_data_dir(&upper), None);
        let beside = root.path().join("MacOS");
        std::fs::create_dir_all(&beside).unwrap();
        std::fs::write(beside.join(PORTABLE_FILE), "").unwrap();
        assert_eq!(portable_data_dir(&beside), Some(beside.join("data")));
        assert_eq!(portable_data_dir(root.path()), None, "без portable.txt");
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
