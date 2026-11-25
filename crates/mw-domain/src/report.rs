//! Формулы отчёта по заказам: признак «без ремонта» и оплата мастеру.

use crate::money::Money;
use crate::search::normalize_search;

/// Заказ закрыт «без ремонта»: текст «Выполненных работ» делится на части по `,`, `;` и переводам
/// строки, у каждой части убираются регистр, `ё`, лишние пробелы и точки в конце; признак есть,
/// если хоть одна часть равна «без ремонта».
pub fn is_without_repair(work_done: &str) -> bool {
    work_done
        .split([',', ';', '\n', '\r'])
        .map(|part| {
            normalize_search(part)
                .trim_end_matches(['.', ' '])
                .to_owned()
        })
        .any(|part| part == "без ремонта")
}

/// Оплата мастеру: `profit` × `percent` / 100 с округлением до копейки половиной от нуля.
/// Убыточный заказ даёт отрицательную сумму. Переполнение невозможно: промежуточное значение
/// считается в `i128`, итог сводится к границе `i64`.
pub fn master_pay(profit: Money, percent: u8) -> Money {
    let scaled = i128::from(profit.kopecks()) * i128::from(percent);
    let mut kopecks = scaled / 100;
    if (scaled % 100).abs() >= 50 {
        kopecks += scaled.signum();
    }
    Money::from_kopecks(i64::try_from(kopecks).unwrap_or(if kopecks < 0 {
        i64::MIN
    } else {
        i64::MAX
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_repair_reads_the_work_done_text() {
        for yes in [
            "Без ремонта",
            "без ремонта,",
            "БЕЗ РЕМОНТА\r\n",
            "Диагностика, без ремонта",
            "Без ремонта. ",
            "Без ремонта; клиент отказался",
        ] {
            assert!(is_without_repair(yes), "{yes:?}");
        }
        for no in ["", "Ремонт без замены платы", "Замена дисплея"]
        {
            assert!(!is_without_repair(no), "{no:?}");
        }
    }

    #[test]
    fn master_pay_rounds_half_away_from_zero() {
        let pay = |profit: i64, percent: u8| master_pay(Money::from_kopecks(profit), percent);
        assert_eq!(pay(101, 50), Money::from_kopecks(51));
        assert_eq!(pay(-101, 50), Money::from_kopecks(-51));
        assert_eq!(pay(1000, 0), Money::ZERO);
        assert_eq!(pay(1000, 100), Money::from_kopecks(1000));
        assert_eq!(pay(333, 33), Money::from_kopecks(110));
    }
}
