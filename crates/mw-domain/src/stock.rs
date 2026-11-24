//! Склад: пределы позиции, выводимое наименование, «Докупить» и сумма запчасти заказа.

use crate::money::Money;

/// Сколько фото может быть у позиции склада.
pub const MAX_STOCK_PHOTOS: usize = 3;
/// Наибольший остаток позиции и порог «Докупить при».
pub const MAX_STOCK_QUANTITY: i64 = 1_000_000;
/// Наибольшая цена позиции за штуку. С `MAX_STOCK_QUANTITY` сумма запчасти не переполняется.
pub const MAX_STOCK_PRICE: Money = Money::from_rubles(10_000_000);

/// Поля позиции, из которых выводится наименование, — в порядке вывода.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StockNameParts<'a> {
    pub part_type: &'a str,
    pub device_type: &'a str,
    pub brand: &'a str,
    pub model: &'a str,
    pub color: &'a str,
}

/// Непустые «тип запчасти, тип устройства, бренд, модель, цвет» (без пробелов по краям) через
/// пробел; все пусты — пустая строка.
pub fn derived_name(parts: &StockNameParts<'_>) -> String {
    [
        parts.part_type,
        parts.device_type,
        parts.brand,
        parts.model,
        parts.color,
    ]
    .iter()
    .map(|part| part.trim())
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join(" ")
}

/// Наименование позиции для людей: введённое (без пробелов по краям), иначе выведенное из полей
/// (`derived_name`), иначе «Позиция №{id}».
pub fn display_name(name: &str, parts: &StockNameParts<'_>, id: i64) -> String {
    let name = name.trim();
    if !name.is_empty() {
        return name.to_owned();
    }
    let derived = derived_name(parts);
    if derived.is_empty() {
        format!("Позиция №{id}")
    } else {
        derived
    }
}

/// «Докупить»: порог задан (больше нуля), а остаток ниже него.
pub fn needs_restock(quantity: i64, threshold: i64) -> bool {
    threshold > 0 && quantity < threshold
}

/// Сумма запчасти заказа: цена за штуку × количество; переполнение — `None`.
pub fn part_sum(price: Money, quantity: i64) -> Option<Money> {
    price
        .kopecks()
        .checked_mul(quantity)
        .map(Money::from_kopecks)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts<'a>(
        part_type: &'a str,
        device_type: &'a str,
        brand: &'a str,
        model: &'a str,
        color: &'a str,
    ) -> StockNameParts<'a> {
        StockNameParts {
            part_type,
            device_type,
            brand,
            model,
            color,
        }
    }

    #[test]
    fn display_name_is_the_entered_name_then_the_parts_then_the_number() {
        let display = parts("Дисплей", "Телефон", "", "A52", "Чёрный");
        assert_eq!(display_name("  Экран  ", &display, 7), "Экран");
        assert_eq!(display_name("", &display, 7), "Дисплей Телефон A52 Чёрный");
        assert_eq!(derived_name(&display), "Дисплей Телефон A52 Чёрный");
        let blank = parts("", " ", "", "", "");
        assert_eq!(derived_name(&blank), "");
        assert_eq!(display_name("   ", &blank, 7), "Позиция №7");
    }

    #[test]
    fn restock_only_below_a_positive_threshold() {
        assert!(needs_restock(3, 5));
        assert!(!needs_restock(5, 5));
        assert!(!needs_restock(0, 0));
        assert!(!needs_restock(9, 0));
    }

    #[test]
    fn part_sum_multiplies_and_refuses_overflow() {
        assert_eq!(
            part_sum(Money::from_rubles(2500), 2),
            Some(Money::from_rubles(5000))
        );
        assert_eq!(part_sum(Money::from_kopecks(i64::MAX), 2), None);
    }

    #[test]
    fn limits_hold_the_whole_range_without_overflow() {
        assert_eq!(MAX_STOCK_PHOTOS, 3);
        assert_eq!(MAX_STOCK_QUANTITY, 1_000_000);
        assert_eq!(MAX_STOCK_PRICE, Money::from_rubles(10_000_000));
        assert!(part_sum(MAX_STOCK_PRICE, MAX_STOCK_QUANTITY).is_some());
    }
}
