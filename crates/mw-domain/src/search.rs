//! Нормализация текста для регистронезависимого поиска (колонки `search_text`).

use unicode_normalization::UnicodeNormalization;

const MAX_NUMBER_DIGITS: usize = 10;
const MIN_PHONE_DIGITS: usize = 5;

/// Сначала NFC (составные символы, «й» и «ё» одним кодом), затем нижний регистр
/// Unicode, `ё` -> `е`, любые пробельные символы схлопнуты в один пробел,
/// края обрезаны.
pub fn normalize_search(input: &str) -> String {
    let lowered: String = input
        .nfc()
        .flat_map(char::to_lowercase)
        .map(|c| if c == 'ё' { 'е' } else { c })
        .collect();
    lowered.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Буквы русской раскладки ЙЦУКЕН, переведённые в латинские по положению клавиш
/// (регистр сохраняется; остальные знаки не меняются).
pub fn qwerty_from_jcuken(s: &str) -> String {
    const RU: &str = "йцукенгшщзфывапролдячсмить";
    const EN: &str = "qwertyuiopasdfghjklzxcvbnm";
    s.chars()
        .map(|c| {
            let lower = c.to_lowercase().next().unwrap_or(c);
            match RU.chars().position(|r| r == lower) {
                Some(i) => {
                    let latin = EN.chars().nth(i).unwrap_or(c);
                    if c.is_uppercase() {
                        latin.to_ascii_uppercase()
                    } else {
                        latin
                    }
                }
                None => c,
            }
        })
        .collect()
}

/// Нормализованные слова запроса без повторов, в порядке появления.
///
/// Правила для цифр:
/// - запрос целиком из цифр, пробелов и `+-().` с минимум 5 цифрами --
///   один токен из всех цифр (телефон);
/// - слово только из цифр и `+-()` превращается в свои цифры;
/// - токен из одних цифр длиннее 10 укорачивается до последних 10 цифр
///   (суффикс остаётся подстрокой числа: IMEI и штрихкоды находятся,
///   а `8…` и `+7…` совпадают).
pub fn search_tokens(input: &str) -> Vec<String> {
    let normalized = normalize_search(input);
    let mut tokens: Vec<String> = Vec::new();
    let candidates: Vec<String> = if is_phone_like_query(&normalized) {
        vec![digits_of(&normalized)]
    } else {
        normalized.split(' ').map(collapse_numeric_word).collect()
    };
    for token in candidates {
        let token = shorten_number(token);
        if !token.is_empty() && !tokens.contains(&token) {
            tokens.push(token);
        }
    }
    tokens
}

/// Экранирует `\`, `%`, `_` обратной косой для `LIKE ... ESCAPE '\'`.
fn escape_like(text: &str, out: &mut String) {
    for c in text.chars() {
        if matches!(c, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(c);
    }
}

/// `%` + токен с экранированием `\`, `%`, `_` обратной косой + `%`.
/// Запросы всегда используют `ESCAPE '\'`.
pub fn like_contains(token: &str) -> String {
    let mut out = String::with_capacity(token.len() + 2);
    out.push('%');
    escape_like(token, &mut out);
    out.push('%');
    out
}

/// Шаблон «начинается с» для кодов действий журнала: префикс приводится к нижнему регистру
/// (коды — строчный ASCII; `LIKE` в SQLite не различает регистр ASCII, в PostgreSQL различает,
/// поэтому без этого результаты на бэкендах расходятся), экранируется и получает `%` в конце.
/// Запросы всегда используют `ESCAPE '\'`.
pub fn like_prefix(prefix: &str) -> String {
    let mut out = String::with_capacity(prefix.len() + 1);
    escape_like(&prefix.to_lowercase(), &mut out);
    out.push('%');
    out
}

/// Text for `search_text` columns: each field normalized with `normalize_search`,
/// plus the digit-only forms that `search_tokens` would produce for numbers
/// written with separators, so a query typed exactly like the stored value
/// always matches. Fields empty after normalization are skipped. No last-10
/// shortening here: the full stored digits contain the query's suffix.
pub fn index_text(fields: &[&str]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for field in fields {
        let normalized = normalize_search(field);
        if normalized.is_empty() {
            continue;
        }
        if is_phone_like_query(&normalized) {
            let digits = digits_of(&normalized);
            if digits != normalized {
                parts.push(normalized);
                parts.push(digits);
            } else {
                parts.push(normalized);
            }
            continue;
        }
        let extra: Vec<String> = normalized
            .split(' ')
            .filter(|w| is_separated_number(w))
            .map(digits_of)
            .collect();
        parts.push(normalized);
        parts.extend(extra);
    }
    parts.join(" ")
}

fn digits_of(s: &str) -> String {
    s.chars().filter(char::is_ascii_digit).collect()
}

fn is_phone_like_query(s: &str) -> bool {
    s.chars()
        .all(|c| c.is_ascii_digit() || c == ' ' || "+-().".contains(c))
        && s.chars().filter(char::is_ascii_digit).count() >= MIN_PHONE_DIGITS
}

fn is_numeric_word(word: &str) -> bool {
    word.chars()
        .all(|c| c.is_ascii_digit() || "+-()".contains(c))
}

/// Numeric word that has at least one digit and at least one separator.
fn is_separated_number(word: &str) -> bool {
    is_numeric_word(word)
        && word.chars().any(|c| c.is_ascii_digit())
        && word.chars().any(|c| !c.is_ascii_digit())
}

fn collapse_numeric_word(word: &str) -> String {
    if is_numeric_word(word) {
        digits_of(word)
    } else {
        word.to_owned()
    }
}

fn shorten_number(token: String) -> String {
    if token.len() > MAX_NUMBER_DIGITS && token.chars().all(|c| c.is_ascii_digit()) {
        token[token.len() - MAX_NUMBER_DIGITS..].to_owned()
    } else {
        token
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jcuken_maps_by_key_position() {
        assert_eq!(
            qwerty_from_jcuken("йцукенгшщзфывапролдячсмить"),
            "qwertyuiopasdfghjklzxcvbnm"
        );
        assert_eq!(qwerty_from_jcuken("ЙЦУ-2"), "QWE-2");
        assert_eq!(qwerty_from_jcuken("хъ"), "хъ");
    }

    #[test]
    fn normalize_search_composes_unicode_first() {
        assert_eq!(normalize_search("Пе\u{0308}тр"), "петр"); // «ё» двумя кодами
        assert_eq!(normalize_search("И\u{0306}ошкар-Ола"), "йошкар-ола");
        assert_eq!(search_tokens("ПЕ\u{0308}ТР"), vec!["петр"]);
        assert_eq!(index_text(&["Пе\u{0308}тр"]), "петр");
    }

    #[test]
    fn normalize_search_folds_case_yo_and_spaces() {
        assert_eq!(normalize_search("  Пётр   ИВАНОВ "), "петр иванов");
        assert_eq!(normalize_search("Ёлка\tЁЖ\nX"), "елка еж x");
        assert_eq!(normalize_search("Samsung A52"), "samsung a52");
    }

    #[test]
    fn search_tokens_split_dedup_and_handle_numbers() {
        assert_eq!(search_tokens("Петров  a52 ПЕТРОВ"), vec!["петров", "a52"]);
        assert_eq!(search_tokens("   "), Vec::<String>::new());
        assert_eq!(search_tokens("+7 (921) 123-45-67"), vec!["9211234567"]);
        assert_eq!(search_tokens("8 921 123 45 67"), vec!["9211234567"]);
        assert_eq!(
            search_tokens("Петров 8-921-123-45-67"),
            vec!["петров", "9211234567"]
        );
        assert_eq!(search_tokens("356789104512345"), vec!["9104512345"]);
        assert_eq!(search_tokens("10481"), vec!["10481"]);
        assert_eq!(search_tokens("50% o'neil"), vec!["50%", "o'neil"]);
    }

    #[test]
    fn like_contains_escapes_wildcards() {
        assert_eq!(like_contains("50%"), r"%50\%%");
        assert_eq!(like_contains("a_b"), r"%a\_b%");
        assert_eq!(like_contains(r"c:\x"), r"%c:\\x%");
    }

    #[test]
    fn like_prefix_lowercases_and_escapes() {
        assert_eq!(like_prefix("ORDER."), "order.%");
        assert_eq!(like_prefix("a_b%"), r"a\_b\%%");
        assert_eq!(like_prefix(r"x\y"), r"x\\y%");
        assert_eq!(like_prefix(""), "%");
    }

    #[test]
    fn index_text_adds_digit_forms_for_numbers_with_separators() {
        assert_eq!(
            index_text(&["ПЁТР ИВАНОВ", "79211234567"]),
            "петр иванов 79211234567"
        );
        assert_eq!(index_text(&["1234-5678"]), "1234-5678 12345678");
        assert_eq!(index_text(&["SN 12-34 X"]), "sn 12-34 x 1234");
        assert_eq!(index_text(&["", "  "]), "");
    }

    #[test]
    fn every_query_token_is_found_in_index_of_same_text() {
        for q in [
            "1234-5678",
            "1234 5678",
            "12-34",
            "+7 (921) 123-45-67",
            "Samsung A52",
            "Пётр 8-921-123-45-67",
        ] {
            let idx = index_text(&[q]);
            for t in search_tokens(q) {
                assert!(idx.contains(&t), "{q:?}: token {t:?} not in {idx:?}");
            }
        }
    }
}
