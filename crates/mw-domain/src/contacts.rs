//! Контакты для телефонной книги: номера, CSV для Google Контактов и vCard 3.0.

use crate::phone::normalize_phone;

/// Один контакт: имя и номера в порядке клиента.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contact {
    pub name: String,
    pub phones: Vec<String>,
}

/// Самый короткий номер, который попадает в телефонную книгу.
const MIN_DIGITS: usize = 5;
/// Сколько байт в одной строке vCard (RFC 2425), не считая CRLF.
const FOLD_OCTETS: usize = 75;

/// Номер в виде для телефонной книги: российский (10 цифр, 11 с 7 или 8 в начале) — `+7XXXXXXXXXX`,
/// остальные — цифры как есть; короче 5 цифр — `None`.
pub fn contact_phone(stored: &str) -> Option<String> {
    let digits = normalize_phone(stored);
    if digits.len() < MIN_DIGITS {
        return None;
    }
    let national = match digits.len() {
        10 => Some(digits.as_str()),
        11 if digits.starts_with(['7', '8']) => digits.get(1..),
        _ => None,
    };
    Some(match national {
        Some(n) => format!("+7{n}"),
        None => digits,
    })
}

/// CSV для импорта в Google Контакты: UTF-8, CRLF, кавычки по RFC 4180.
pub fn contacts_csv(contacts: &[Contact]) -> String {
    let max = contacts
        .iter()
        .map(|c| c.phones.len())
        .max()
        .unwrap_or(0)
        .max(1);
    let mut header = vec![
        "Name".to_owned(),
        "Given Name".into(),
        "Group Membership".into(),
    ];
    for n in 1..=max {
        header.push(format!("Phone {n} - Type"));
        header.push(format!("Phone {n} - Value"));
    }
    let mut out = String::new();
    push_csv_row(&mut out, &header);
    for c in contacts {
        let mut row = vec![c.name.clone(), c.name.clone(), "* myContacts".to_owned()];
        for n in 0..max {
            match c.phones.get(n) {
                Some(p) => {
                    row.push("Mobile".to_owned());
                    row.push(p.clone());
                }
                None => row.extend([String::new(), String::new()]),
            }
        }
        push_csv_row(&mut out, &row);
    }
    out
}

fn push_csv_row(out: &mut String, fields: &[String]) {
    for (i, f) in fields.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        if f.contains([',', '"', '\r', '\n']) {
            out.push('"');
            out.push_str(&f.replace('"', "\"\""));
            out.push('"');
        } else {
            out.push_str(f);
        }
    }
    out.push_str("\r\n");
}

/// vCard 3.0: по карточке на контакт, CRLF, экранирование и перенос строк длиннее 75 байт.
pub fn contacts_vcard(contacts: &[Contact]) -> String {
    let mut out = String::new();
    for c in contacts {
        let name = vcard_escape(&c.name);
        push_folded(&mut out, "BEGIN:VCARD");
        push_folded(&mut out, "VERSION:3.0");
        push_folded(&mut out, &format!("FN:{name}"));
        push_folded(&mut out, &format!("N:;{name};;;"));
        for p in &c.phones {
            push_folded(&mut out, &format!("TEL;TYPE=CELL:{}", vcard_escape(p)));
        }
        push_folded(&mut out, "END:VCARD");
    }
    out
}

fn vcard_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => out.push_str("\\\\"),
            ';' => out.push_str(r"\;"),
            ',' => out.push_str("\\,"),
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push_str("\\n");
            }
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out
}

/// Строка с переносами: не больше 75 байт на физическую строку (продолжение начинается пробелом,
/// он входит в счёт), символ UTF-8 не разрезается.
fn push_folded(out: &mut String, line: &str) {
    let mut used = 0;
    for ch in line.chars() {
        let size = ch.len_utf8();
        if used + size > FOLD_OCTETS {
            out.push_str("\r\n ");
            used = 1;
        }
        out.push(ch);
        used += size;
    }
    out.push_str("\r\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contact(name: &str, phones: &[&str]) -> Contact {
        Contact {
            name: name.into(),
            phones: phones.iter().map(|p| (*p).into()).collect(),
        }
    }

    #[test]
    fn contact_phones_for_a_phone_book() {
        let cases = [
            ("8 (921) 123-45-67", Some("+79211234567")),
            ("9211234567", Some("+79211234567")),
            ("+7 921 123 45 67", Some("+79211234567")),
            ("123-45-67", Some("1234567")),
            ("12", None),
        ];
        for (stored, expected) in cases {
            assert_eq!(contact_phone(stored).as_deref(), expected, "{stored}");
        }
    }

    #[test]
    fn csv_quotes_what_needs_quoting() {
        let one = contacts_csv(&[contact("Иванов, \"Ваня\"", &["+79211234567"])]);
        assert_eq!(
            one,
            "Name,Given Name,Group Membership,Phone 1 - Type,Phone 1 - Value\r\n\
             \"Иванов, \"\"Ваня\"\"\",\"Иванов, \"\"Ваня\"\"\",* myContacts,Mobile,+79211234567\r\n"
        );
        let two = contacts_csv(&[
            contact("Петров", &["+79211234567"]),
            contact("Сидоров", &["1234567", "+79217654321"]),
        ]);
        assert_eq!(
            two,
            "Name,Given Name,Group Membership,Phone 1 - Type,Phone 1 - Value,Phone 2 - Type,Phone 2 - Value\r\n\
             Петров,Петров,* myContacts,Mobile,+79211234567,,\r\n\
             Сидоров,Сидоров,* myContacts,Mobile,1234567,Mobile,+79217654321\r\n"
        );
        let broken = contacts_csv(&[contact("Строка\nвторая\rтретья", &["1234567"])]);
        assert!(broken.contains("\"Строка\nвторая\rтретья\""));
    }

    #[test]
    fn vcard_escapes_and_folds() {
        let card = contacts_vcard(&[contact(
            "Петров; ООО, \"Север\"\nсклад\\",
            &["+79211234567", "1234567"],
        )]);
        assert_eq!(
            card,
            "BEGIN:VCARD\r\nVERSION:3.0\r\n\
             FN:Петров\\; ООО\\, \"Север\"\\nсклад\\\\\r\n\
             N:;Петров\\; ООО\\, \"Север\"\\nсклад\\\\;;;\r\n\
             TEL;TYPE=CELL:+79211234567\r\nTEL;TYPE=CELL:1234567\r\nEND:VCARD\r\n"
        );

        let long = "Ё".repeat(60);
        let card = contacts_vcard(&[contact(&long, &["1234567"])]);
        assert!(card.ends_with("\r\n"));
        let physical: Vec<&str> = card.strip_suffix("\r\n").unwrap().split("\r\n").collect();
        for line in &physical {
            assert!(line.len() <= 75, "{} байт: {line}", line.len());
        }
        assert!(physical.iter().any(|l| l.starts_with(' ')));
        // Склейка продолжений возвращает исходную строку, символы целы.
        let unfolded = card.replace("\r\n ", "");
        assert!(unfolded.contains(&format!("FN:{long}\r\n")));
        assert!(unfolded.contains(&format!("N:;{long};;;\r\n")));
    }
}
