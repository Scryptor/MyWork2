//! Платежи по заказу: этап относительно выдачи и границы суммы. Чистые функции без ввода-вывода.

use crate::money::Money;

code_enum! {
    /// Этап платежа относительно выдачи заказа.
    PaymentStage {
        BeforeIssue => "before_issue",
        AtIssue => "at_issue",
        AfterIssue => "after_issue",
    }
}

/// Наибольшая сумма одного платежа: 999 999 999,99 ₽.
pub const MAX_PAYMENT: Money = Money::from_kopecks(99_999_999_999);

/// Наибольшая длина поля «Принял».
pub const MAX_RECEIVED_BY_CHARS: usize = 100;

/// Сумма платежа — от одной копейки до [`MAX_PAYMENT`].
pub fn amount_in_range(m: Money) -> bool {
    Money::ZERO < m && m <= MAX_PAYMENT
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn stages_have_stable_codes() {
        for (stage, code) in [
            (PaymentStage::BeforeIssue, "before_issue"),
            (PaymentStage::AtIssue, "at_issue"),
            (PaymentStage::AfterIssue, "after_issue"),
        ] {
            assert_eq!(stage.as_str(), code);
            assert_eq!(PaymentStage::from_str(code), Ok(stage));
            assert_eq!(
                serde_json::to_string(&stage).unwrap(),
                format!("\"{code}\"")
            );
            assert_eq!(
                serde_json::from_str::<PaymentStage>(&format!("\"{code}\"")).unwrap(),
                stage
            );
        }
        assert!(PaymentStage::from_str("later").is_err());
    }

    #[test]
    fn amounts_are_from_one_kopeck_to_the_limit() {
        assert!(!amount_in_range(Money::ZERO));
        assert!(!amount_in_range(Money::from_kopecks(-1)));
        assert!(amount_in_range(Money::from_kopecks(1)));
        assert!(amount_in_range(MAX_PAYMENT));
        assert!(!amount_in_range(Money::from_kopecks(
            MAX_PAYMENT.kopecks() + 1
        )));
    }
}
