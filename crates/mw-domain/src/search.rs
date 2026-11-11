//! Нормализация текста для регистронезависимого поиска (колонки `search_text`).

const MAX_NUMBER_DIGITS: usize = 10;
const MIN_PHONE_DIGITS: usize = 5;

/// Нижний регистр Unicode, `ё` -> `е`, любые пробельные символы схлопнуты
/// в один пробел, края обрезаны.
pub fn normalize_search(input: &str) -> String {
    let lowered: String = input
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| if c == 'ё' { 'е' } else { c })
        .collect();
    lowered.split_whitespace().collect::<Vec<_>>().join(" ")
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

/// `%` + токен с экранированием `\`, `%`, `_` обратной косой + `%`.
/// Запросы всегда используют `ESCAPE '\'`.
pub fn like_contains(token: &str) -> String {
    let mut out = String::with_capacity(token.len() + 2);
    out.push('%');
    for c in token.chars() {
        if matches!(c, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('%');
    out
}

fn digits_of(s: &str) -> String {
    s.chars().filter(char::is_ascii_digit).collect()
}

fn is_phone_like_query(s: &str) -> bool {
    s.chars()
        .all(|c| c.is_ascii_digit() || c == ' ' || "+-().".contains(c))
        && s.chars().filter(char::is_ascii_digit).count() >= MIN_PHONE_DIGITS
}

fn collapse_numeric_word(word: &str) -> String {
    if word
        .chars()
        .all(|c| c.is_ascii_digit() || "+-()".contains(c))
    {
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
}
