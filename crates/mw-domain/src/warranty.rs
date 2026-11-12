//! Срок гарантии, выведенный из текста («30 дней», «2 года», «без гарантии»).

use serde::{Deserialize, Serialize};

use crate::search::normalize_search;

code_enum! {
    /// Единица срока гарантии.
    WarrantyUnit { Days => "days", Months => "months" }
}

/// Наибольший срок в днях.
pub const MAX_WARRANTY_DAYS: u16 = 3650;
/// Наибольший срок в месяцах.
pub const MAX_WARRANTY_MONTHS: u16 = 120;

/// Срок гарантии; `value = 0` -- «без гарантии».
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema,
)]
pub struct WarrantyTerm {
    pub unit: WarrantyUnit,
    pub value: u16,
}

impl WarrantyTerm {
    pub fn days(value: u16) -> Self {
        Self {
            unit: WarrantyUnit::Days,
            value,
        }
    }

    pub fn months(value: u16) -> Self {
        Self {
            unit: WarrantyUnit::Months,
            value,
        }
    }
}

/// Выводит срок из текста гарантии. Составные и непонятные тексты дают `None`.
pub fn parse_warranty_term(text: &str) -> Option<WarrantyTerm> {
    let normalized = normalize_search(text);
    let words: Vec<&str> = normalized
        .split(' ')
        .map(|w| w.trim_end_matches('.'))
        .collect();
    let joined = words.join(" ");
    let mut rest = joined.trim();
    if let Some(after) = rest.strip_prefix("гарантия") {
        rest = after.trim_start_matches([':', '-', ' ']);
    }
    if rest.is_empty() {
        return None;
    }
    if matches!(
        rest,
        "без гарантии" | "без гар" | "нет" | "нет гарантии" | "-" | "0"
    ) {
        return Some(WarrantyTerm::days(0));
    }
    let digits_end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    let (digits, unit) = rest.split_at(digits_end);
    let number: u16 = digits.parse().ok()?;
    let (kind, factor) = match unit.trim_start() {
        "" | "д" | "дн" | "день" | "дня" | "дней" | "сут" | "суток" | "сутки" => {
            (WarrantyUnit::Days, 1)
        }
        "нед" | "неделя" | "недели" | "недель" => (WarrantyUnit::Days, 7),
        "мес" | "месяц" | "месяца" | "месяцев" => (WarrantyUnit::Months, 1),
        "г" | "год" | "года" | "лет" => (WarrantyUnit::Months, 12),
        _ => return None,
    };
    let value = number.checked_mul(factor)?;
    let max = match kind {
        WarrantyUnit::Days => MAX_WARRANTY_DAYS,
        WarrantyUnit::Months => MAX_WARRANTY_MONTHS,
    };
    (value <= max).then_some(WarrantyTerm { unit: kind, value })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_warranty_texts() {
        let t = parse_warranty_term;
        assert_eq!(t("30 дней"), Some(WarrantyTerm::days(30)));
        assert_eq!(t("3 дня"), Some(WarrantyTerm::days(3)));
        assert_eq!(t("90"), Some(WarrantyTerm::days(90)));
        assert_eq!(t("2 недели"), Some(WarrantyTerm::days(14)));
        assert_eq!(t("Гарантия: 14 дн."), Some(WarrantyTerm::days(14)));
        assert_eq!(t("3 мес"), Some(WarrantyTerm::months(3)));
        assert_eq!(t("6 мес."), Some(WarrantyTerm::months(6)));
        assert_eq!(t("4 месяцев"), Some(WarrantyTerm::months(4)));
        assert_eq!(t("1 год"), Some(WarrantyTerm::months(12)));
        assert_eq!(t("2 года"), Some(WarrantyTerm::months(24)));
        for none in ["Без гарантии", "без гарантии", "0", "нет", "-"] {
            assert_eq!(t(none), Some(WarrantyTerm::days(0)), "{none}");
        }
    }

    #[test]
    fn composite_or_unknown_warranty_has_no_term() {
        for s in [
            "",
            "  ",
            "До установки.",
            "30 дней - чистка, 6 месяцев блок питания",
            "ОС-30 дней; ssd-3 года",
            "99999 дней",
            "121 месяц",
            "три месяца",
        ] {
            assert_eq!(parse_warranty_term(s), None, "{s}");
        }
    }

    #[test]
    fn warranty_term_json_uses_codes() {
        assert_eq!(
            serde_json::to_string(&WarrantyTerm::months(3)).unwrap(),
            r#"{"unit":"months","value":3}"#
        );
    }
}
