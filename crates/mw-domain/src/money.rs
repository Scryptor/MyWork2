//! Денежный тип проекта: сумма в копейках (`i64`), без чисел с плавающей точкой.

use std::borrow::Cow;
use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Сумма денег в копейках.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    Serialize,
    Deserialize,
    ts_rs::TS,
    utoipa::ToSchema,
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

    /// Сумма без знака: тысячи через `separator`, запятая перед копейками, если они есть.
    fn unsigned_text(self, separator: char) -> String {
        let abs = self.0.unsigned_abs();
        let (rubles, kopecks) = (abs / 100, abs % 100);
        let digits = rubles.to_string();
        let mut out = String::new();
        for (i, ch) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i) % 3 == 0 {
                out.push(separator);
            }
            out.push(ch);
        }
        if kopecks != 0 {
            out.push_str(&format!(",{kopecks:02}"));
        }
        out
    }

    /// Форматирует для показа: тысячи через неразрывный пробел, запятая
    /// перед копейками; копейки опускаются, если их нет.
    pub fn format_ru(self) -> String {
        let mut out = String::new();
        if self.0 < 0 {
            out.push('-');
        }
        out.push_str(&self.unsigned_text('\u{a0}'));
        out
    }
}

/// Знаки валют, которых нет в шрифте документов, и чем они печатаются (тест покрытия шрифта — в
/// `mw-core`, `tests/documents.rs`).
pub const DOC_CURRENCY_REPLACEMENTS: [(&str, &str); 3] = [("₽", "руб."), ("₸", "тг"), ("₴", "грн")];

/// Текст для печатного документа: каждый знак из [`DOC_CURRENCY_REPLACEMENTS`] — словом, остальное
/// как есть. В словах замен знаков нет, так что повторный вызов ничего не меняет.
pub fn doc_text(text: &str) -> Cow<'_, str> {
    if !DOC_CURRENCY_REPLACEMENTS
        .iter()
        .any(|(sign, _)| text.contains(sign))
    {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len() + 8);
    for c in text.chars() {
        let mut buf = [0; 4];
        let sign = &*c.encode_utf8(&mut buf);
        match DOC_CURRENCY_REPLACEMENTS.iter().find(|(s, _)| *s == sign) {
            Some((_, word)) => out.push_str(word),
            None => out.push(c),
        }
    }
    Cow::Owned(out)
}

/// Сумма для печатного документа: как `formatMoney` в `apps/ui/src/lib/format/money.ts` (примеры в
/// тестах обеих сторон общие), но знак из [`DOC_CURRENCY_REPLACEMENTS`] заменяется словом
/// ([`doc_text`]); пустой `symbol` — без знака.
pub fn format_money_doc(m: Money, symbol: &str) -> String {
    let mut text = String::new();
    if m.kopecks() < 0 {
        text.push('\u{2212}');
    }
    text.push_str(&m.unsigned_text('\u{202f}'));
    if !symbol.is_empty() {
        text.push('\u{a0}');
        text.push_str(&doc_text(symbol));
    }
    text
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

    /// Общие примеры с `formatMoney` (`apps/ui/src/lib/format/money.test.ts`): знаки, которые есть в
    /// шрифте документов, печатаются как в интерфейсе.
    const FORMAT_MONEY_EXAMPLES: [(i64, &str, &str); 7] = [
        (650_000, "$", "6\u{202f}500\u{a0}$"),
        (650_050, "$", "6\u{202f}500,50\u{a0}$"),
        (0, "€", "0\u{a0}€"),
        (5, "$", "0,05\u{a0}$"),
        (12_345_678_900, "$", "123\u{202f}456\u{202f}789\u{a0}$"),
        (-50_000, "$", "\u{2212}500\u{a0}$"),
        (650_000, "", "6\u{202f}500"),
    ];

    #[test]
    fn format_money_doc_matches_the_interface() {
        for (kopecks, symbol, shown) in FORMAT_MONEY_EXAMPLES {
            let m = Money::from_kopecks(kopecks);
            assert_eq!(format_money_doc(m, symbol), shown, "{kopecks} {symbol}");
        }
    }

    #[test]
    fn format_money_doc_replaces_signs_the_font_lacks() {
        let m = Money::from_rubles(1500);
        assert_eq!(format_money_doc(m, "₽"), "1\u{202f}500\u{a0}руб.");
        assert_eq!(format_money_doc(m, "₸"), "1\u{202f}500\u{a0}тг");
        assert_eq!(format_money_doc(m, "₴"), "1\u{202f}500\u{a0}грн");
        assert_eq!(format_money_doc(m, "руб."), "1\u{202f}500\u{a0}руб.");
        assert_eq!(format_money_doc(m, "Br"), "1\u{202f}500\u{a0}Br");
    }

    #[test]
    fn doc_text_replaces_every_sign_the_font_lacks_and_only_once() {
        assert_eq!(
            doc_text("Оплата 500 ₽, 2000 ₸ или 70₴."),
            "Оплата 500 руб., 2000 тг или 70грн."
        );
        assert_eq!(doc_text("₽₽"), "руб.руб.");
        for text in ["Без знаков: $ € руб.", "", "₽ и ₸", "₴"] {
            let once = doc_text(text);
            assert_eq!(doc_text(&once), once, "{text:?}");
        }
        assert!(matches!(doc_text("1 500 $"), std::borrow::Cow::Borrowed(_)));
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
