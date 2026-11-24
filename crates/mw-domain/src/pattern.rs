//! Графический ключ устройства: точки сетки 3 × 3 с номерами 1–9 (слева направо, сверху вниз) в
//! порядке ввода, строкой цифр — «1589». Пустой ключ — ключа нет.

/// Меньше точек в ключе не бывает.
pub const PATTERN_MIN_POINTS: usize = 4;

/// Точек в сетке, а значит и в самом длинном ключе.
const PATTERN_MAX_POINTS: usize = 9;

/// Проверяет ключ: пусто или от 4 до 9 цифр 1–9 без повторов. Ошибка — сообщение для человека.
pub fn validate_pattern(text: &str) -> Result<(), &'static str> {
    if text.is_empty() {
        return Ok(());
    }
    if !text.bytes().all(|b| (b'1'..=b'9').contains(&b)) {
        return Err("Графический ключ — точки с номерами от 1 до 9");
    }
    if text.len() > PATTERN_MAX_POINTS {
        return Err("В графическом ключе не больше 9 точек");
    }
    let mut seen = [false; PATTERN_MAX_POINTS];
    for b in text.bytes() {
        let point = usize::from(b - b'1');
        if std::mem::replace(&mut seen[point], true) {
            return Err("Точка графического ключа не повторяется");
        }
    }
    if text.len() < PATTERN_MIN_POINTS {
        return Err("В графическом ключе не меньше 4 точек");
    }
    Ok(())
}

/// Ключ текстом для документа: «1 · 5 · 8 · 9»; пустой — пусто.
pub fn pattern_text(text: &str) -> String {
    let points: Vec<String> = text.chars().map(String::from).collect();
    points.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_four_to_nine_distinct_points_are_valid() {
        for ok in ["", "1589", "123456789"] {
            assert_eq!(validate_pattern(ok), Ok(()), "{ok:?}");
        }
    }

    #[test]
    fn short_repeated_foreign_and_long_patterns_are_invalid() {
        // 3 точки, повтор, `0`, буква, 10 знаков.
        for bad in ["158", "1123", "1580", "a123", "1234567891"] {
            assert!(validate_pattern(bad).is_err(), "{bad:?}");
        }
        assert_eq!(PATTERN_MIN_POINTS, 4);
    }

    #[test]
    fn pattern_prints_its_points_joined_by_middle_dots() {
        assert_eq!(pattern_text("158963"), "1 · 5 · 8 · 9 · 6 · 3");
        assert_eq!(pattern_text(""), "");
    }
}
