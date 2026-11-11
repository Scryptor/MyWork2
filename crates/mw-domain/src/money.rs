//! Денежный тип проекта: сумма в копейках (`i64`), без чисел с плавающей точкой.

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Сумма денег в копейках.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Money(i64);

/// Ошибка разбора суммы, введённой человеком.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum MoneyParseError {
    #[error("сумма не указана")]
    Empty,
    #[error("некорректная сумма")]
    Invalid,
    #[error("сумма не может быть отрицательной")]
    Negative,
    #[error("копеек может быть не больше двух знаков")]
    TooManyDecimals,
    #[error("слишком большая сумма")]
    Overflow,
}

impl Money {
    pub const ZERO: Money = Money(0);

    pub const fn from_kopecks(kopecks: i64) -> Money {
        Money(kopecks)
    }

    /// Создаёт сумму из целых рублей. При переполнении `i64` результат
    /// насыщается до границы диапазона (паники нет).
    pub const fn from_rubles(rubles: i64) -> Money {
        Money(rubles.saturating_mul(100))
    }

    pub const fn kopecks(self) -> i64 {
        self.0
    }

    /// Разбирает сумму в русских форматах: `6500`, `6 500`, `6\u{a0}500,50`,
    /// `6500.5`, с суффиксом `₽` / `р` / `р.` / `руб` / `руб.`.
    pub fn parse(input: &str) -> Result<Money, MoneyParseError> {
        let compact: String = input
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '\u{a0}' && *c != '\u{202f}')
            .flat_map(char::to_lowercase)
            .collect();
        let mut text = compact.as_str();
        for suffix in ["руб.", "руб", "р.", "р", "₽"] {
            if let Some(rest) = text.strip_suffix(suffix) {
                text = rest;
                break;
            }
        }
        if text.is_empty() {
            return Err(MoneyParseError::Empty);
        }
        if text.starts_with('-') {
            return Err(MoneyParseError::Negative);
        }

        let mut separators = text.match_indices([',', '.']);
        let (int_part, frac_part) = match (separators.next(), separators.next()) {
            (None, _) => (text, ""),
            (Some((at, _)), None) => (&text[..at], &text[at + 1..]),
            _ => return Err(MoneyParseError::Invalid),
        };
        let all_digits = |s: &str| s.chars().all(|c| c.is_ascii_digit());
        if int_part.is_empty()
            || !all_digits(int_part)
            || !all_digits(frac_part)
            || (text.len() > int_part.len() && frac_part.is_empty())
        {
            return Err(MoneyParseError::Invalid);
        }
        if frac_part.len() > 2 {
            return Err(MoneyParseError::TooManyDecimals);
        }

        let rubles: i64 = int_part.parse().map_err(|_| MoneyParseError::Overflow)?;
        let frac: i64 = match frac_part.len() {
            0 => 0,
            1 => {
                frac_part
                    .parse::<i64>()
                    .map_err(|_| MoneyParseError::Invalid)?
                    * 10
            }
            _ => frac_part.parse().map_err(|_| MoneyParseError::Invalid)?,
        };
        rubles
            .checked_mul(100)
            .and_then(|k| k.checked_add(frac))
            .map(Money)
            .ok_or(MoneyParseError::Overflow)
    }

    /// Форматирует для показа: тысячи через неразрывный пробел, запятая
    /// перед копейками; копейки опускаются, если их нет.
    pub fn format_ru(self) -> String {
        let abs = self.0.unsigned_abs();
        let (rubles, kopecks) = (abs / 100, abs % 100);
        let digits = rubles.to_string();
        let mut out = String::new();
        if self.0 < 0 {
            out.push('-');
        }
        for (i, ch) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i) % 3 == 0 {
                out.push('\u{a0}');
            }
            out.push(ch);
        }
        if kopecks != 0 {
            out.push_str(&format!(",{kopecks:02}"));
        }
        out
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.format_ru())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_accepts_russian_formats() {
        assert_eq!(Money::parse("6500").unwrap().kopecks(), 650_000);
        assert_eq!(Money::parse("6 500").unwrap().kopecks(), 650_000);
        assert_eq!(Money::parse("6\u{a0}500,50").unwrap().kopecks(), 650_050);
        assert_eq!(Money::parse("6\u{202f}500.5").unwrap().kopecks(), 650_050);
        assert_eq!(Money::parse(" 6 500 ₽ ").unwrap().kopecks(), 650_000);
        assert_eq!(Money::parse("6500 р.").unwrap().kopecks(), 650_000);
        assert_eq!(Money::parse("6500руб").unwrap().kopecks(), 650_000);
        assert_eq!(Money::parse("0").unwrap(), Money::ZERO);
    }

    #[test]
    fn parse_rejects_bad_input() {
        assert_eq!(Money::parse(""), Err(MoneyParseError::Empty));
        assert_eq!(Money::parse("  ₽ "), Err(MoneyParseError::Empty));
        assert_eq!(Money::parse("-100"), Err(MoneyParseError::Negative));
        assert_eq!(
            Money::parse("6500.555"),
            Err(MoneyParseError::TooManyDecimals)
        );
        assert_eq!(Money::parse("6,500.50"), Err(MoneyParseError::Invalid));
        assert_eq!(Money::parse("abc"), Err(MoneyParseError::Invalid));
        assert_eq!(
            Money::parse("99999999999999999999"),
            Err(MoneyParseError::Overflow)
        );
    }

    #[test]
    fn format_ru_groups_thousands_with_nbsp() {
        assert_eq!(Money::from_kopecks(650_000).format_ru(), "6\u{a0}500");
        assert_eq!(Money::from_kopecks(650_050).format_ru(), "6\u{a0}500,50");
        assert_eq!(Money::from_kopecks(5).format_ru(), "0,05");
        assert_eq!(
            Money::from_kopecks(12_345_678_900).format_ru(),
            "123\u{a0}456\u{a0}789"
        );
    }

    #[test]
    fn serde_is_plain_kopecks() {
        assert_eq!(
            serde_json::to_string(&Money::from_kopecks(650_050)).unwrap(),
            "650050"
        );
        assert_eq!(
            serde_json::from_str::<Money>("650050").unwrap(),
            Money::from_kopecks(650_050)
        );
    }
}
