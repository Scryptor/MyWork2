//! Настройки из `Config.ini` и текстов актов, а также сохранение файлов в `legacy_rows`.

use mw_core::{ImportedLegacyRow, ImportedSettings, SettingsPatch};
use serde_json::json;

use crate::convert::catalogs::Catalogs;
use crate::folder::LegacyFolder;
use crate::parse::parse_flag;
use crate::report::{WarningCode, Warnings};
use crate::text::{Ini, TextEncoding, decode_text, parse_ini, text_lines};

/// Секции, которые не относятся к компьютеру.
const SHARED_SECTIONS: &[&str] = &[
    "ACTS",
    "BARCODE",
    "CHECKBOX",
    "PROGRAMM_SETTINGS",
    "SMSSEND",
    "EXPORT",
    "ACTIVATION",
];
/// Секции, которые не читаются вообще (токен SMS, ключ активации): убираются сразу после разбора.
const SECRET_SECTIONS: &[&str] = &["SMSSEND", "ACTIVATION"];
/// Ключи с соответствием: (секция или `*` для секции компьютера, ключ).
const MAPPED_KEYS: &[(&str, &str)] = &[
    ("PROGRAMM_SETTINGS", "valuta"),
    ("PROGRAMM_SETTINGS", "Mpersent"),
    ("PROGRAMM_SETTINGS", "daysDiagnostik"),
    ("PROGRAMM_SETTINGS", "NessesaryCHB"),
    ("CHECKBOX", "garantyDefault"),
    ("*", "AdressSCDefault"),
    ("*", "MasterDefault"),
];

/// Файлы актов: (файл, настройка, заготовка MyWork 2 или её начало).
const ACT_FILES: &[(&str, &str, Option<&str>)] = &[
    ("FirmName.txt", "firm_name", Some("Название Вашей Фирмы")),
    ("Phone.txt", "firm_phone", Some("тел.: Вашей фирмы")),
    ("DannieOFirme.txt", "firm_details", None),
    (
        "URDannie.txt",
        "firm_legal_details",
        Some("ИП Какойктото В.Е.,"),
    ),
    ("DogovorTextPriem.txt", "intake_terms", None),
    ("DogovorTextVidacha.txt", "issue_terms", None),
];

fn is_secret(section: &str) -> bool {
    SECRET_SECTIONS
        .iter()
        .any(|s| s.eq_ignore_ascii_case(section))
}

fn is_shared(section: &str) -> bool {
    SHARED_SECTIONS
        .iter()
        .any(|s| s.eq_ignore_ascii_case(section))
}

fn encoding_name(encoding: TextEncoding) -> &'static str {
    match encoding {
        TextEncoding::Utf8Bom => "utf-8-bom",
        TextEncoding::Utf16LeBom => "utf-16le-bom",
        TextEncoding::Utf8 => "utf-8",
        TextEncoding::Windows1251 => "windows-1251",
    }
}

/// `<br>` перед переводом строки становится переводом строки; края обрезаются.
fn clean_act_text(text: &str) -> String {
    let unified = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut out = String::with_capacity(unified.len());
    let mut rest = unified.as_str();
    while let Some(pos) = rest.find('<') {
        out.push_str(&rest[..pos]);
        let tail = &rest[pos..];
        let br_len = ["<br>", "<br/>", "<br />"]
            .iter()
            .find(|tag| {
                tail.get(..tag.len())
                    .is_some_and(|t| t.eq_ignore_ascii_case(tag))
            })
            .map(|tag| tag.len());
        match br_len {
            Some(len) if tail[len..].starts_with('\n') => rest = &tail[len..],
            _ => {
                out.push('<');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out.trim().to_owned()
}

/// Флажок `Checked`/`Unchecked` в любом регистре.
fn checkbox(value: &str) -> Option<bool> {
    parse_flag(value)
}

struct Invalid<'a> {
    w: &'a mut Warnings,
}

impl Invalid<'_> {
    fn report(&mut self, section: &str, key: &str, value: &str, why: &str) {
        self.w.push(
            WarningCode::SettingInvalid,
            "Config.ini",
            &format!("[{section}] {key}"),
            key,
            value,
            &format!("{why} Настройка осталась стандартной."),
        );
    }
}

fn apply_program_settings(ini: &Ini, patch: &mut SettingsPatch, bad: &mut Invalid) {
    let get = |key| ini.get("PROGRAMM_SETTINGS", key);
    if let Some(v) = get("valuta") {
        let n = v.chars().count();
        if (1..=8).contains(&n) {
            patch.currency_symbol = Some(v.to_owned());
        } else {
            bad.report(
                "PROGRAMM_SETTINGS",
                "valuta",
                v,
                "Знак валюты — от 1 до 8 символов.",
            );
        }
    }
    if let Some(v) = get("Mpersent") {
        match v.parse::<u8>() {
            Ok(n) if n <= 100 => patch.master_percent = Some(n),
            _ => bad.report(
                "PROGRAMM_SETTINGS",
                "Mpersent",
                v,
                "Процент мастера — число от 0 до 100.",
            ),
        }
    }
    if let Some(v) = get("daysDiagnostik") {
        match v.parse::<i32>().map(i32::unsigned_abs) {
            Ok(n @ 1..=365) => patch.diagnostics_overdue_days = Some(n as u16),
            _ => bad.report(
                "PROGRAMM_SETTINGS",
                "daysDiagnostik",
                v,
                "Срок диагностики — число от 1 до 365.",
            ),
        }
    }
    if let Some(v) = get("NessesaryCHB") {
        match checkbox(v) {
            Some(on) => patch.intake_required_fields = Some(on),
            None => bad.report(
                "PROGRAMM_SETTINGS",
                "NessesaryCHB",
                v,
                "Ожидалось Checked или Unchecked.",
            ),
        }
    }
    if let Some(v) = ini.get("CHECKBOX", "garantyDefault") {
        match checkbox(v) {
            Some(true) => patch.default_warranty = Some("Без гарантии".to_owned()),
            Some(false) => {}
            None => bad.report(
                "CHECKBOX",
                "garantyDefault",
                v,
                "Ожидалось Checked или Unchecked.",
            ),
        }
    }
}

fn apply_computer_settings(
    ini: &Ini,
    section: &str,
    folder: &LegacyFolder,
    catalogs: &Catalogs,
    settings: &mut ImportedSettings,
    bad: &mut Invalid,
) {
    if let Some(v) = ini.get(section, "MasterDefault")
        && !v.is_empty()
    {
        settings.default_master = Some(v.to_owned());
    }
    let Some(v) = ini
        .get(section, "AdressSCDefault")
        .filter(|v| !v.is_empty())
    else {
        return;
    };
    let line = v.parse::<usize>().ok().and_then(|index| {
        let bytes = folder.text_files.get("settings/AdresSC.txt")?;
        text_lines(&decode_text(bytes).0).into_iter().nth(index)
    });
    match line.and_then(|l| catalogs.point_for(&l)) {
        Some(point) => settings.default_service_point = Some(point),
        None => bad.report(
            section,
            "AdressSCDefault",
            v,
            "Номер строки AdresSC.txt не указывает на точку.",
        ),
    }
}

fn convert_config(
    folder: &LegacyFolder,
    catalogs: &Catalogs,
    settings: &mut ImportedSettings,
    rows: &mut Vec<ImportedLegacyRow>,
    w: &mut Warnings,
) {
    let Some(bytes) = &folder.config_ini else {
        return;
    };
    let mut ini = parse_ini(&decode_text(bytes).0);
    ini.sections.retain(|s| !is_secret(&s.name));
    let computer = ini
        .sections
        .iter()
        .find(|s| !is_shared(&s.name))
        .map(|s| s.name.clone());
    let mut ignored = std::collections::BTreeSet::new();
    for section in &ini.sections {
        if !is_shared(&section.name)
            && !computer
                .as_deref()
                .is_some_and(|c| c.eq_ignore_ascii_case(&section.name))
            && ignored.insert(section.name.to_lowercase())
        {
            w.push(
                WarningCode::ConfigSectionIgnored,
                "Config.ini",
                &section.name,
                "",
                "",
                "Настройки другого компьютера не использованы, сохранены как есть.",
            );
        }
    }
    apply_program_settings(&ini, &mut settings.values, &mut Invalid { w });
    if let Some(name) = &computer {
        apply_computer_settings(&ini, name, folder, catalogs, settings, &mut Invalid { w });
    }
    let mut seen_keys: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for section in &ini.sections {
        for (key, value) in &section.entries {
            let base_key = format!("[{}] {key}", section.name);
            let count = seen_keys.entry(base_key.to_lowercase()).or_insert(0);
            *count += 1;
            // Повтор ключа (или секции) не теряется, но получает свой ключ; в настройках первый побеждает.
            let source_key = if *count == 1 {
                base_key
            } else {
                format!("{base_key} #{count}")
            };
            let mapped = MAPPED_KEYS.iter().any(|(s, k)| {
                k.eq_ignore_ascii_case(key)
                    && if *s == "*" {
                        computer
                            .as_deref()
                            .is_some_and(|c| c.eq_ignore_ascii_case(&section.name))
                    } else {
                        s.eq_ignore_ascii_case(&section.name)
                    }
            });
            let other_computer =
                !is_shared(&section.name) && computer.as_deref() != Some(section.name.as_str());
            if !mapped && !other_computer {
                w.push(
                    WarningCode::SettingPreserved,
                    "Config.ini",
                    &source_key,
                    key,
                    value,
                    "Настройки с таким ключом в MyWork 3 пока нет, значение сохранено.",
                );
            }
            rows.push(ImportedLegacyRow {
                source: "Config.ini".to_owned(),
                source_key,
                target: None,
                raw: json!({ "section": section.name, "key": key, "value": value }),
            });
        }
    }
}

fn convert_acts(folder: &LegacyFolder, values: &mut SettingsPatch, w: &mut Warnings) {
    for (file, setting, placeholder) in ACT_FILES {
        let path = format!("settings/Akts/{file}");
        let Some(bytes) = folder.text_files.get(&path) else {
            continue;
        };
        let (raw, encoding) = decode_text(bytes);
        if encoding == TextEncoding::Utf8 {
            w.push(
                WarningCode::TextDecodedUtf8,
                "file",
                &path,
                "",
                "",
                "Файл не в Windows-1251, прочитан как UTF-8.",
            );
        }
        let text = clean_act_text(&raw);
        if text.is_empty() {
            continue;
        }
        if placeholder.is_some_and(|p| text.starts_with(p)) {
            w.push(
                WarningCode::ActTextDefaultSkipped,
                "file",
                &path,
                "",
                "",
                "Заготовка MyWork 2 не перенесена.",
            );
            continue;
        }
        let slot = match *setting {
            "firm_name" => &mut values.firm_name,
            "firm_phone" => &mut values.firm_phone,
            "firm_details" => &mut values.firm_details,
            "firm_legal_details" => &mut values.firm_legal_details,
            "intake_terms" => &mut values.intake_terms,
            _ => &mut values.issue_terms,
        };
        *slot = Some(text);
    }
}

/// Настройки и следы файлов настроек. `SMSSEND` и `ACTIVATION` не читаются.
pub(crate) fn convert_settings(
    folder: &LegacyFolder,
    catalogs: &Catalogs,
    w: &mut Warnings,
) -> (ImportedSettings, Vec<ImportedLegacyRow>) {
    let mut settings = ImportedSettings::default();
    let mut rows = Vec::new();
    convert_config(folder, catalogs, &mut settings, &mut rows, w);
    convert_acts(folder, &mut settings.values, w);
    for (path, bytes) in &folder.text_files {
        let (text, encoding) = decode_text(bytes);
        rows.push(ImportedLegacyRow {
            source: "file".to_owned(),
            source_key: path.clone(),
            target: None,
            raw: json!({ "encoding": encoding_name(encoding), "text": text }),
        });
    }
    (settings, rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::convert::catalogs::convert_catalogs;
    use crate::fixture::{FIXTURE_SMS_TOKEN, FixtureVersion::V32};
    use crate::report::Warnings;
    use crate::test_support::{codes, prepared};

    #[tokio::test]
    async fn settings_come_from_config_and_act_texts() {
        let (db, folder) = prepared(V32).await;
        let mut w = Warnings::default();
        let c = convert_catalogs(&db, &folder, &|id| [1, 3, 6].contains(&id), &mut w);
        let (s, rows) = convert_settings(&folder, &c, &mut w);
        let v = &s.values;
        assert_eq!(
            (
                v.currency_symbol.as_deref(),
                v.master_percent,
                v.diagnostics_overdue_days,
                v.intake_required_fields
            ),
            (Some("руб."), Some(40), Some(5), Some(false))
        );
        assert_eq!(
            (
                v.default_warranty.as_deref(),
                s.default_service_point.as_deref(),
                s.default_master.as_deref()
            ),
            (Some("Без гарантии"), Some("Ленина, 1"), Some("ПЕТРОВ"))
        );
        assert_eq!(
            (v.firm_name.as_deref(), v.firm_phone.as_deref()),
            (Some("СЕРВИС-ПЛЮС"), Some("8 (800) 000-00-00"))
        );
        assert_eq!(
            v.firm_details.as_deref(),
            Some("Режим работы: 10–19\nг. Город, ул. Примерная, 1")
        );
        assert_eq!(
            v.intake_terms.as_deref(),
            Some("Строка договора 1\nСтрока договора 2")
        );
        assert_eq!(
            (v.timezone.clone(), v.default_service_point_id),
            (None, None)
        );
        assert_eq!(rows.iter().filter(|r| r.source == "Config.ini").count(), 16);
        assert_eq!(rows.iter().filter(|r| r.source == "file").count(), 13);
        assert!(!format!("{rows:?}").contains(FIXTURE_SMS_TOKEN));
        assert!(!format!("{:?}", w.list()).contains(FIXTURE_SMS_TOKEN));
        assert_eq!(
            w.list()
                .iter()
                .filter(|x| x.code.as_str() == "setting_preserved")
                .count(),
            8
        );
        assert!(codes(&w).contains(&("config_section_ignored", "S2Z9")));
    }

    #[tokio::test]
    async fn repeated_keys_and_sections_keep_unique_row_keys_and_first_wins() {
        let (db, mut folder) = prepared(V32).await;
        let mut w = Warnings::default();
        let c = convert_catalogs(&db, &folder, &|_| false, &mut w);
        folder.config_ini = Some(
            "[PROGRAMM_SETTINGS]\nMpersent=10\nmpersent=20\nNessesaryCHB=checked\n[programm_settings]\nMpersent=30\n[CHECKBOX]\ngarantyDefault=CHECKED\n"
                .as_bytes()
                .to_vec(),
        );
        let mut w = Warnings::default();
        let (s, rows) = convert_settings(&folder, &c, &mut w);
        let keys: Vec<_> = rows
            .iter()
            .filter(|r| r.source == "Config.ini")
            .map(|r| r.source_key.as_str())
            .collect();
        let unique: std::collections::BTreeSet<_> = keys.iter().collect();
        assert_eq!(keys.len(), 5);
        assert_eq!(unique.len(), 5, "{keys:?}");
        assert!(keys.contains(&"[PROGRAMM_SETTINGS] Mpersent"));
        assert!(keys.contains(&"[PROGRAMM_SETTINGS] mpersent #2"));
        assert!(keys.contains(&"[programm_settings] Mpersent #3"));
        assert_eq!(s.values.master_percent, Some(10));
        assert_eq!(s.values.intake_required_fields, Some(true));
        assert_eq!(s.values.default_warranty.as_deref(), Some("Без гарантии"));
    }
}
