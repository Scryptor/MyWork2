//! Нормализация телефонных номеров для поиска и сопоставления.

/// Оставляет в строке только цифры.
pub fn normalize_phone(input: &str) -> String {
    input.chars().filter(char::is_ascii_digit).collect()
}

/// Ключ сопоставления: последние 10 цифр номера, если цифр 10 и больше,
/// иначе все цифры. Так `+7…`, `8…` и номер без кода страны совпадают.
pub fn phone_key(input: &str) -> String {
    let digits = normalize_phone(input);
    last_digits(&digits, 10).to_owned()
}

fn last_digits(digits: &str, n: usize) -> &str {
    // Цифры ASCII, поэтому длина в байтах равна длине в символах.
    &digits[digits.len().saturating_sub(n)..]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phone_key_matches_7_8_and_short_forms() {
        assert_eq!(normalize_phone("+7 (921) 123-45-67"), "79211234567");
        assert_eq!(phone_key("+7 (921) 123-45-67"), "9211234567");
        assert_eq!(phone_key("8 921 123 45 67"), "9211234567");
        assert_eq!(phone_key("9211234567"), "9211234567");
        assert_eq!(phone_key("12-34-5"), "12345");
        assert_eq!(phone_key(""), "");
    }
}
