//! Кодировки, строки и INI-файлы старой программы.

/// Как был закодирован текстовый файл.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextEncoding {
    Utf8Bom,
    Utf16LeBom,
    Utf8,
    Windows1251,
}

/// Читает текст: по BOM; иначе корректный UTF-8 хотя бы с одним не-ASCII символом; иначе Windows-1251.
pub(crate) fn decode_text(bytes: &[u8]) -> (String, TextEncoding) {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return (
            String::from_utf8_lossy(rest).into_owned(),
            TextEncoding::Utf8Bom,
        );
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        let (text, _) = encoding_rs::UTF_16LE.decode_without_bom_handling(rest);
        return (text.into_owned(), TextEncoding::Utf16LeBom);
    }
    if !bytes.is_ascii()
        && let Ok(text) = std::str::from_utf8(bytes)
    {
        return (text.to_owned(), TextEncoding::Utf8);
    }
    let (text, _) = encoding_rs::WINDOWS_1251.decode_without_bom_handling(bytes);
    (text.into_owned(), TextEncoding::Windows1251)
}

/// Строки как у `StreamReader.ReadLine`: разделители `\r\n`, `\n`, `\r`; без пустой строки в конце.
pub(crate) fn text_lines(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut chars = text.chars().peekable();
    let mut pending = false;
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                lines.push(std::mem::take(&mut current));
                pending = false;
            }
            '\n' => {
                lines.push(std::mem::take(&mut current));
                pending = false;
            }
            _ => {
                current.push(c);
                pending = true;
            }
        }
    }
    if pending {
        lines.push(current);
    }
    lines
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Ini {
    pub sections: Vec<IniSection>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct IniSection {
    pub name: String,
    pub entries: Vec<(String, String)>,
}

impl Ini {
    /// Значение ключа без учёта регистра; при повторе побеждает первый ключ.
    pub(crate) fn get(&self, section: &str, key: &str) -> Option<&str> {
        self.sections
            .iter()
            .filter(|s| {
                s.name.eq_ignore_ascii_case(section)
                    || s.name.to_lowercase() == section.to_lowercase()
            })
            .flat_map(|s| s.entries.iter())
            .find(|(k, _)| k.to_lowercase() == key.to_lowercase())
            .map(|(_, v)| v.as_str())
    }
}

/// Разбирает INI как `GetPrivateProfileString`: `[секция]`, `ключ=значение` по первому `=`, `;` — комментарий.
pub(crate) fn parse_ini(text: &str) -> Ini {
    let mut ini = Ini::default();
    for line in text_lines(text) {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|l| l.split_once(']'))
            .map(|(n, _)| n)
        {
            ini.sections.push(IniSection {
                name: name.trim().to_owned(),
                entries: Vec::new(),
            });
        } else if let Some((key, value)) = line.split_once('=')
            && let Some(section) = ini.sections.last_mut()
        {
            section
                .entries
                .push((key.trim().to_owned(), value.trim().to_owned()));
        }
    }
    ini
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_text_detects_bom_utf8_and_cp1251() {
        let (cp1251, _, _) = encoding_rs::WINDOWS_1251.encode("Ремонт");
        assert_eq!(
            decode_text(&cp1251),
            ("Ремонт".to_owned(), TextEncoding::Windows1251)
        );
        assert_eq!(
            decode_text("\u{feff}Ремонт".as_bytes()),
            ("Ремонт".to_owned(), TextEncoding::Utf8Bom)
        );
        assert_eq!(
            decode_text("Ремонт".as_bytes()),
            ("Ремонт".to_owned(), TextEncoding::Utf8)
        );
        assert_eq!(
            decode_text(b"\xff\xfeA\x00"),
            ("A".to_owned(), TextEncoding::Utf16LeBom)
        );
        assert_eq!(
            decode_text(b"plain"),
            ("plain".to_owned(), TextEncoding::Windows1251)
        );
    }

    #[test]
    fn text_lines_follow_readline() {
        assert_eq!(text_lines("a\r\nb\n\nc\r"), ["a", "b", "", "c"]);
        assert_eq!(text_lines("a\r\n"), ["a"]);
    }

    #[test]
    fn ini_is_case_insensitive_and_first_key_wins() {
        let ini = parse_ini("[A]\nk=1\nK=2\n; комментарий\n[b]\n x = y z \n");
        assert_eq!(ini.get("a", "K"), Some("1"));
        assert_eq!(ini.get("B", "X"), Some("y z"));
    }
}
