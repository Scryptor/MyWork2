//! Просмотр папки старой программы: только чтение.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::LegacyError;

/// Имена текстовых файлов настроек со своим написанием, как в коде MyWork 2.
const KNOWN_SETTINGS: &[&str] = &[
    "AdresSC.txt",
    "DeviceColour.txt",
    "Garanty.txt",
    "aboutUs.txt",
    "brands.txt",
    "komplektonst.txt",
    "masters.txt",
    "neispravnost.txt",
    "sostoyaniePriema.txt",
    "ustrojstvo.txt",
    "vipolnRaboti.txt",
];
const KNOWN_ACTS: &[&str] = &[
    "DannieOFirme.txt",
    "DogovorTextPriem.txt",
    "DogovorTextVidacha.txt",
    "FirmName.txt",
    "Phone.txt",
    "URDannie.txt",
];
const KNOWN_STOCK: &[&str] = &[
    "DeviceColour.txt",
    "TypeOvZIP.txt",
    "brands.txt",
    "ustrojstvo.txt",
];
const KNOWN_TEMPLATES: &[&str] = &[
    "Shablon_Act_Priema.htm",
    "Shablon_Act_vidachi.htm",
    "Shablon_act_priemaGarantii.htm",
    "Shablon_act_vidachiGarantii.htm",
    "Shablon_Priem.htm",
];

/// Что найдено в папке старой программы.
#[derive(Debug, Clone)]
pub(crate) struct LegacyFolder {
    pub root: PathBuf,
    pub db_file: PathBuf,
    pub config_ini: Option<Vec<u8>>,
    /// Канонический путь (`settings/brands.txt`, `Shablon_Priem.htm`, …) → байты файла.
    pub text_files: BTreeMap<String, Vec<u8>>,
    /// Пути внутри `ClientFiles` через `/`, по алфавиту.
    pub client_files: Vec<String>,
}

impl LegacyFolder {
    /// Ищет файл по относительному пути без учёта регистра в каждом сегменте (разделители `/` и `\`).
    ///
    /// Путь приходит из содержимого старой базы, поэтому он не должен выводить за пределы папки: пустые сегменты
    /// (в том числе ведущий разделитель), `.`, `..` и сегменты с `:` (диск `C:`) дают `None`, а найденный файл
    /// обязан лежать внутри `root` (после раскрытия ссылок).
    pub(crate) fn find_file(&self, relative: &str) -> Option<PathBuf> {
        let mut current = self.root.clone();
        for segment in relative.split(['/', '\\']) {
            if segment.is_empty() || segment == "." || segment == ".." || segment.contains(':') {
                return None;
            }
            current = find_child(&current, segment)?;
        }
        let root = self.root.canonicalize().ok()?;
        let found = current.canonicalize().ok()?;
        (found.is_file() && found.starts_with(&root)).then_some(current)
    }
}

fn io_error(context: String) -> impl FnOnce(std::io::Error) -> LegacyError {
    move |source| LegacyError::Io { context, source }
}

/// Дочерний элемент папки с именем без учёта регистра (точное совпадение предпочтительнее).
fn find_child(dir: &Path, name: &str) -> Option<PathBuf> {
    let exact = dir.join(name);
    if exact.exists() {
        return Some(exact);
    }
    let wanted = name.to_lowercase();
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().to_lowercase() == wanted)
        .map(|e| e.path())
        .collect();
    found.sort();
    found.into_iter().next()
}

fn find_dir(dir: &Path, name: &str) -> Option<PathBuf> {
    find_child(dir, name).filter(|p| p.is_dir())
}

fn read_file(path: &Path) -> Result<Vec<u8>, LegacyError> {
    std::fs::read(path).map_err(io_error(format!(
        "Не удалось прочитать файл {}",
        path.display()
    )))
}

/// Обычные файлы папки (без вложенных папок и ссылок), по имени.
fn list_files(dir: &Path) -> Result<Vec<(String, PathBuf)>, LegacyError> {
    let mut files = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(io_error(format!(
        "Не удалось прочитать папку {}",
        dir.display()
    )))?;
    for entry in entries {
        let entry = entry.map_err(io_error(format!(
            "Не удалось прочитать папку {}",
            dir.display()
        )))?;
        if entry.file_type().is_ok_and(|t| t.is_file()) {
            files.push((
                entry.file_name().to_string_lossy().into_owned(),
                entry.path(),
            ));
        }
    }
    files.sort();
    Ok(files)
}

fn canonical_name(known: &[&str], disk_name: &str) -> String {
    known
        .iter()
        .find(|k| k.to_lowercase() == disk_name.to_lowercase())
        .map_or_else(|| disk_name.to_owned(), |k| (*k).to_owned())
}

fn collect_texts(
    dir: &Path,
    prefix: &str,
    known: &[&str],
    suffix: &str,
    name_filter: impl Fn(&str) -> bool,
    out: &mut BTreeMap<String, Vec<u8>>,
) -> Result<(), LegacyError> {
    for (name, path) in list_files(dir)? {
        let lower = name.to_lowercase();
        if lower.ends_with(suffix) && name_filter(&lower) {
            out.insert(
                format!("{prefix}{}", canonical_name(known, &name)),
                read_file(&path)?,
            );
        }
    }
    Ok(())
}

fn collect_tree(dir: &Path, prefix: &str, out: &mut Vec<String>) -> Result<(), LegacyError> {
    let entries = std::fs::read_dir(dir).map_err(io_error(format!(
        "Не удалось прочитать папку {}",
        dir.display()
    )))?;
    for entry in entries {
        let entry = entry.map_err(io_error(format!(
            "Не удалось прочитать папку {}",
            dir.display()
        )))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            collect_tree(&entry.path(), &format!("{prefix}{name}/"), out)?;
        } else if kind.is_file() {
            out.push(format!("{prefix}{name}"));
        }
    }
    Ok(())
}

/// Просматривает папку старой программы; `source` — папка или файл `baza.sqlite` в ней. Ничего не меняет.
pub(crate) fn scan_folder(source: &Path) -> Result<LegacyFolder, LegacyError> {
    if !source.exists() {
        return Err(LegacyError::SourceNotFound(source.to_path_buf()));
    }
    let (root, db_file) = if source.is_file() {
        let parent = source
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        (parent.to_path_buf(), source.to_path_buf())
    } else {
        let db = find_child(source, "baza.sqlite")
            .filter(|p| p.is_file())
            .ok_or_else(|| LegacyError::NoDatabase(source.to_path_buf()))?;
        (source.to_path_buf(), db)
    };

    let config_ini = match find_child(&root, "Config.ini").filter(|p| p.is_file()) {
        Some(path) => Some(read_file(&path)?),
        None => None,
    };

    let mut text_files = BTreeMap::new();
    collect_texts(
        &root,
        "",
        KNOWN_TEMPLATES,
        ".htm",
        |n| n.starts_with("shablon_"),
        &mut text_files,
    )?;
    if let Some(settings) = find_dir(&root, "settings") {
        collect_texts(
            &settings,
            "settings/",
            KNOWN_SETTINGS,
            ".txt",
            |_| true,
            &mut text_files,
        )?;
        if let Some(acts) = find_dir(&settings, "Akts") {
            collect_texts(
                &acts,
                "settings/Akts/",
                KNOWN_ACTS,
                ".txt",
                |_| true,
                &mut text_files,
            )?;
        }
        if let Some(stock) = find_dir(&settings, "Stock") {
            collect_texts(
                &stock,
                "settings/Stock/",
                KNOWN_STOCK,
                ".txt",
                |_| true,
                &mut text_files,
            )?;
        }
    }

    let mut client_files = Vec::new();
    if let Some(dir) = find_dir(&root, "ClientFiles") {
        collect_tree(&dir, "", &mut client_files)?;
    }
    client_files.sort();

    Ok(LegacyFolder {
        root,
        db_file,
        config_ini,
        text_files,
        client_files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_finds_files_case_insensitively() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        std::fs::write(p.join("BAZA.SQLITE"), b"x").unwrap();
        std::fs::create_dir_all(p.join("Settings/AKTS")).unwrap();
        std::fs::write(p.join("Settings/AKTS/firmname.TXT"), b"x").unwrap();
        std::fs::create_dir_all(p.join("CLIENTFILES/1/1")).unwrap();
        std::fs::write(p.join("CLIENTFILES/1/1/a.jpg"), b"x").unwrap();
        std::fs::create_dir_all(p.join("settings/backup")).unwrap();
        std::fs::write(p.join("settings/backup/old.sqlite"), b"x").unwrap();
        let f = scan_folder(p).unwrap();
        assert!(f.text_files.contains_key("settings/Akts/FirmName.txt"));
        assert_eq!(f.client_files, ["1/1/a.jpg"]);
        assert!(!f.text_files.keys().any(|k| k.contains("backup")));
        assert_eq!(scan_folder(&p.join("BAZA.SQLITE")).unwrap().root, f.root);
    }

    #[test]
    fn find_file_is_case_insensitive_and_stays_inside_the_folder() {
        let outer = tempfile::tempdir().unwrap();
        let root = outer.path().join("old");
        std::fs::create_dir_all(root.join("Settings/Stock/Photos")).unwrap();
        std::fs::write(root.join("baza.sqlite"), b"x").unwrap();
        std::fs::write(root.join("Settings/Stock/Photos/ABC.jpg"), b"x").unwrap();
        std::fs::write(outer.path().join("secret.txt"), b"x").unwrap();
        let f = scan_folder(&root).unwrap();
        let hit = f.find_file("settings\\stock/PHOTOS\\abc.JPG").unwrap();
        assert!(
            hit.to_string_lossy()
                .to_lowercase()
                .ends_with("settings/stock/photos/abc.jpg")
        );
        assert_eq!(f.find_file("settings/stock/photos/missing.jpg"), None);
        assert_eq!(f.find_file("settings/Stock/../../../secret.txt"), None);
        assert_eq!(f.find_file("..\\secret.txt"), None);
        assert_eq!(f.find_file("settings/./Stock/Photos/ABC.jpg"), None);
        assert_eq!(f.find_file("/etc/hosts"), None);
        assert_eq!(f.find_file("\\Windows\\x.jpg"), None);
        assert_eq!(f.find_file("C:\\Photos\\ABC.jpg"), None);
        assert_eq!(
            f.find_file(&outer.path().join("secret.txt").to_string_lossy()),
            None
        );
    }
}
