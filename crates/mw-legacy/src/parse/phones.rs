//! Свободное поле «телефон»: несколько номеров, слова и пометки в одной строке.

use mw_core::ClientPhone;
use mw_domain::phone::phone_key;

/// Итог разбора поля телефона.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PhoneSplit {
    pub phones: Vec<ClientPhone>,
    /// Текст без цифр (например «нет телефона»).
    pub text_without_number: String,
    /// Номера необычной длины (они всё равно в `phones`).
    pub unusual: Vec<String>,
    /// Из поля получилось не ровно один номер или есть пометка.
    pub reshaped: bool,
}

enum Token {
    Digits(String),
    Word(String),
}

/// Слова-связки между номерами.
const LINK_WORDS: [&str; 5] = ["или", "и", "либо", "тел", "т"];

pub(crate) fn split_phones(raw: &str) -> PhoneSplit {
    let mut out = PhoneSplit {
        phones: Vec::new(),
        text_without_number: String::new(),
        unusual: Vec::new(),
        reshaped: false,
    };
    let mut last: Option<usize> = None;
    let mut leading_notes: Vec<String> = Vec::new();
    for token in tokenize(raw) {
        match token {
            Token::Digits(digits) => {
                for number in classify(&digits, &mut out.unusual) {
                    last = Some(add_phone(&mut out.phones, number));
                }
            }
            Token::Word(word) => {
                if LINK_WORDS.contains(&word.to_lowercase().as_str()) {
                    continue;
                }
                match last {
                    Some(i) => append_note(&mut out.phones[i].note, &word),
                    None => leading_notes.push(word),
                }
            }
        }
    }
    if out.phones.is_empty() {
        if raw.chars().any(char::is_alphanumeric) {
            out.text_without_number = raw.trim().to_owned();
        }
    } else {
        for word in leading_notes {
            append_note(&mut out.phones[0].note, &word);
        }
    }
    out.reshaped = out.phones.len() != 1 || out.phones.iter().any(|p| !p.note.is_empty());
    out
}

fn append_note(note: &mut String, word: &str) {
    if !note.is_empty() {
        note.push(' ');
    }
    note.push_str(word);
}

/// Добавляет номер; дубль по `phone_key` не добавляется. Возвращает индекс номера.
fn add_phone(phones: &mut Vec<ClientPhone>, number: String) -> usize {
    let key = phone_key(&number);
    if let Some(i) = phones.iter().position(|p| phone_key(&p.phone) == key) {
        return i;
    }
    phones.push(ClientPhone {
        phone: number,
        note: String::new(),
    });
    phones.len() - 1
}

/// Делит строку на группы цифр и слова. Разделители и прочие знаки закрывают группу.
fn tokenize(raw: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut digits = String::new();
    let mut in_group = false;
    let mut word = String::new();
    let flush_group = |digits: &mut String, tokens: &mut Vec<Token>| {
        if !digits.is_empty() {
            tokens.push(Token::Digits(std::mem::take(digits)));
        }
    };
    let flush_word = |word: &mut String, tokens: &mut Vec<Token>| {
        if !word.is_empty() {
            tokens.push(Token::Word(std::mem::take(word)));
        }
    };
    for c in raw.chars() {
        if c.is_ascii_digit() {
            flush_word(&mut word, &mut tokens);
            digits.push(c);
            in_group = true;
        } else if c.is_alphabetic() {
            flush_group(&mut digits, &mut tokens);
            in_group = false;
            word.push(c);
        } else if matches!(c, ' ' | '\u{a0}' | '\t' | '+' | '-' | '(' | ')' | '.') {
            // Внутри группы эти знаки не мешают; слово они закрывают.
            flush_word(&mut word, &mut tokens);
            let _ = in_group;
        } else {
            // `, ; / \ |`, перевод строки и всё прочее.
            flush_word(&mut word, &mut tokens);
            flush_group(&mut digits, &mut tokens);
            in_group = false;
        }
    }
    flush_word(&mut word, &mut tokens);
    flush_group(&mut digits, &mut tokens);
    tokens
}

fn starts_7_or_8(digits: &str) -> bool {
    digits.starts_with('7') || digits.starts_with('8')
}

/// Делит группу цифр на номера по их числу.
fn classify(digits: &str, unusual: &mut Vec<String>) -> Vec<String> {
    let numbers: Vec<String> = match digits.len() {
        11 if starts_7_or_8(digits) => vec![digits.to_owned()],
        10 | 5..=7 => vec![digits.to_owned()],
        22 if starts_7_or_8(&digits[..11]) && starts_7_or_8(&digits[11..]) => {
            vec![digits[..11].to_owned(), digits[11..].to_owned()]
        }
        17 if starts_7_or_8(&digits[..11]) => {
            vec![digits[..11].to_owned(), digits[11..].to_owned()]
        }
        17 if starts_7_or_8(&digits[6..]) => vec![digits[..6].to_owned(), digits[6..].to_owned()],
        _ => {
            unusual.push(digits.to_owned());
            vec![digits.to_owned()]
        }
    };
    numbers
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nums(s: &PhoneSplit) -> Vec<(&str, &str)> {
        s.phones
            .iter()
            .map(|p| (p.phone.as_str(), p.note.as_str()))
            .collect()
    }

    #[test]
    fn single_numbers() {
        assert_eq!(nums(&split_phones("89211234567")), [("89211234567", "")]);
        assert!(!split_phones("89211234567").reshaped);
        assert_eq!(
            nums(&split_phones("+7 (921) 123-45-67")),
            [("79211234567", "")]
        );
        assert_eq!(
            nums(&split_phones("8-921-123-45-67")),
            [("89211234567", "")]
        );
        assert_eq!(nums(&split_phones("9211234567")), [("9211234567", "")]);
        assert!(split_phones("89211234567").unusual.is_empty());
    }

    #[test]
    fn several_numbers_in_one_field() {
        let two = vec![("89211234567", ""), ("89117654321", "")];
        for s in [
            "89211234567,89117654321",
            "8921123456789117654321",
            "89211234567 или 89117654321",
            "89211234567или89117654321",
            "8 921 123 45 67 8 911 765 43 21",
            "89211234567, 89211234567, 89117654321",
            "89211234567\r\n89117654321",
            "тел. 89211234567 и 89117654321",
        ] {
            assert_eq!(nums(&split_phones(s)), two, "{s}");
        }
        assert!(split_phones("89211234567,89117654321").reshaped);
    }

    #[test]
    fn notes() {
        assert_eq!(
            nums(&split_phones("89211234567жена")),
            [("89211234567", "жена")]
        );
        assert!(split_phones("89211234567жена").reshaped);
        assert_eq!(
            nums(&split_phones("жена 89117654321")),
            [("89117654321", "жена")]
        );
        assert_eq!(
            nums(&split_phones("89211234567/89117654321(раб)")),
            [("89211234567", ""), ("89117654321", "раб")]
        );
    }

    #[test]
    fn city_and_glued_numbers() {
        assert_eq!(
            nums(&split_phones("89211234567, 123456")),
            [("89211234567", ""), ("123456", "")]
        );
        assert_eq!(
            nums(&split_phones("89211234567123456")),
            [("89211234567", ""), ("123456", "")]
        );
        assert_eq!(
            nums(&split_phones("123456,89211234567")),
            [("123456", ""), ("89211234567", "")]
        );
    }

    #[test]
    fn unusual_numbers_are_kept_and_reported() {
        let short = split_phones("1234");
        assert_eq!(
            (nums(&short), short.unusual.clone()),
            (vec![("1234", "")], vec!["1234".to_owned()])
        );
        let odd = split_phones("12345678901");
        assert_eq!(odd.unusual, vec!["12345678901".to_owned()]);
        assert_eq!(nums(&odd), [("12345678901", "")]);
    }

    #[test]
    fn placeholders_and_text() {
        for empty in ["", " ", "-", "?", " - ? "] {
            let s = split_phones(empty);
            assert!(
                s.phones.is_empty() && s.text_without_number.is_empty(),
                "{empty:?}"
            );
        }
        let words = split_phones("нет телефона");
        assert!(words.phones.is_empty());
        assert_eq!(words.text_without_number, "нет телефона");
    }
}
