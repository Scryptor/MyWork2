//! Суммы из старой базы.

use mw_domain::money::Money;

/// Сумма больше этого числа рублей переносится как есть, с предупреждением.
pub(crate) const SUSPICIOUS_RUBLES: i64 = 1_000_000;

/// Результат разбора суммы.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MoneyValue {
    Empty,
    Parsed(Money),
    Invalid,
}

/// Разбирает сумму через `Money::parse`; отрицательные и прочие ошибки — `Invalid`.
pub(crate) fn parse_money(raw: &str) -> MoneyValue {
    let text = raw.replace('\u{a0}', " ");
    let text = text.trim();
    if text.is_empty() {
        return MoneyValue::Empty;
    }
    match Money::parse(text) {
        Ok(m) => MoneyValue::Parsed(m),
        Err(_) => MoneyValue::Invalid,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_old_money_strings() {
        assert_eq!(
            parse_money("1500"),
            MoneyValue::Parsed(Money::from_rubles(1500))
        );
        assert_eq!(
            parse_money(" 1 500 "),
            MoneyValue::Parsed(Money::from_rubles(1500))
        );
        assert_eq!(
            parse_money("1\u{a0}500"),
            MoneyValue::Parsed(Money::from_rubles(1500))
        );
        assert_eq!(
            parse_money("1500,50"),
            MoneyValue::Parsed(Money::from_kopecks(150_050))
        );
        assert_eq!(
            parse_money("1500р"),
            MoneyValue::Parsed(Money::from_rubles(1500))
        );
        assert_eq!(parse_money(""), MoneyValue::Empty);
        assert_eq!(parse_money(" \u{a0}"), MoneyValue::Empty);
    }

    #[test]
    fn rejects_garbage() {
        for bad in ["abc", "-100", "1500.555"] {
            assert_eq!(parse_money(bad), MoneyValue::Invalid, "{bad}");
        }
    }
}
