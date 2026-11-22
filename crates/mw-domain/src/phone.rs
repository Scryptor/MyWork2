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

/// Номер для показа и печати: российский (10 цифр, 11 с 7 или 8 в начале) — «+7 921 123-45-67»,
/// городской из 6 цифр — «12-34-56», остальное как есть. То же, что `formatPhone` в
/// `apps/ui/src/lib/format/phone.ts`; примеры в тестах обеих сторон общие.
pub fn format_phone(digits: &str) -> String {
    let all: Vec<char> = digits.chars().collect();
    let mut d = all.as_slice();
    if d.len() == 11 && matches!(d.first(), Some('7' | '8')) {
        d = &d[1..];
    }
    let text = |part: &[char]| part.iter().collect::<String>();
    if d.len() == 10 {
        return format!(
            "+7 {} {}-{}-{}",
            text(&d[..3]),
            text(&d[3..6]),
            text(&d[6..8]),
            text(&d[8..])
        );
    }
    if all.len() == 6 {
        return format!(
            "{}-{}-{}",
            text(&all[..2]),
            text(&all[2..4]),
            text(&all[4..])
        );
    }
    digits.to_owned()
}

fn last_digits(digits: &str, n: usize) -> &str {
    // Цифры ASCII, поэтому длина в байтах равна длине в символах.
    &digits[digits.len().saturating_sub(n)..]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Общие примеры с `formatPhone` (`apps/ui/src/lib/format/phone.test.ts`).
    const FORMAT_PHONE_EXAMPLES: [(&str, &str); 8] = [
        ("89211234567", "+7 921 123-45-67"),
        ("79211234567", "+7 921 123-45-67"),
        ("9211234567", "+7 921 123-45-67"),
        ("123456", "12-34-56"),
        ("12345", "12345"),
        ("19211234567", "19211234567"),
        ("4951234567890", "4951234567890"),
        ("", ""),
    ];

    #[test]
    fn format_phone_matches_the_interface() {
        for (digits, shown) in FORMAT_PHONE_EXAMPLES {
            assert_eq!(format_phone(digits), shown, "{digits}");
        }
    }

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
