//! Штрихкоды заказов: UPC-A на наклейках и разбор отсканированного кода.
//!
//! Контрольная цифра совпадает с `barcodeLastDigit` из MyWork 2, чтобы старые
//! наклейки продолжали читаться. Новые коды имеют вид `"2" + id (10 цифр) + контроль`.
//!
//! В MyWork 2 было два генератора старых кодов (по 11 цифр + контроль):
//! - при приёме нового заказа (`BdWrite`) — по дате: `ddMMHHmmss` + последняя цифра года;
//! - массово для заказов без кода (`bdBarcodeAllGenerator`) — случайные `4+4+3` цифры:
//!   `1111..9998`, `1111..9998`, `333..998`.
//!
//! Почему новый код при `id < 1_000_000` не совпадает со старым (индексы цифр с нуля):
//! у нового кода цифры с индексами 1–4 равны `0000`. У кода по дате цифры 2–3 — месяц
//! `01..12`, то есть никогда не `00`. У случайного кода цифра 4 — первая цифра второго
//! числа `1111..9998`, то есть никогда не `0`. Начиная с 1 000 000 заказов совпадение со
//! случайным старым кодом становится возможным (поиск по штрихкоду и так умеет
//! обрабатывать несколько совпадений). Коды по дате могут повторяться и между собой:
//! при приёме в одну секунду и каждые 10 лет.

/// Наибольший номер заказа, который помещается в 10 цифр штрихкода.
const MAX_ORDER_ID: i64 = 9_999_999_999;

/// Контрольная цифра UPC-A по первым 11 цифрам: цифры на чётных индексах
/// умножаются на 3, остальные берутся как есть, контроль = `(10 - сумма % 10) % 10`.
/// Возвращает `None`, если на входе не ровно 11 ASCII-цифр.
pub fn upc_a_check_digit(first11: &str) -> Option<u8> {
    let bytes = first11.as_bytes();
    if bytes.len() != 11 || !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let sum: u32 = bytes
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let digit = u32::from(*b - b'0');
            if i % 2 == 0 { digit * 3 } else { digit }
        })
        .sum();
    u8::try_from((10 - sum % 10) % 10).ok()
}

/// Проверяет 12-значный UPC-A: ровно 12 ASCII-цифр и верная контрольная цифра.
pub fn is_valid_upc_a(code: &str) -> bool {
    let bytes = code.as_bytes();
    if bytes.len() != 12 || !bytes.iter().all(u8::is_ascii_digit) {
        return false;
    }
    let (body, check) = code.split_at(11);
    let expected = upc_a_check_digit(body);
    matches!((expected, check.as_bytes().first()), (Some(e), Some(c)) if c - b'0' == e)
}

/// Штрихкод заказа: `"2"`, номер заказа, дополненный нулями до 10 цифр, и контрольная цифра.
/// Возвращает `None`, если номер вне диапазона `1..=9_999_999_999`.
pub fn order_barcode(order_id: i64) -> Option<String> {
    if !(1..=MAX_ORDER_ID).contains(&order_id) {
        return None;
    }
    let body = format!("2{order_id:010}");
    let check = upc_a_check_digit(&body)?;
    Some(format!("{body}{check}"))
}

/// Приводит отсканированный код к виду, который ищется в базе.
/// Убирает пробелы и переводы строк. Если остаётся 13 цифр с ведущим `0`
/// (сканер настроен на EAN-13), этот `0` убирается. Остальное не меняется.
pub fn normalize_scanned(input: &str) -> String {
    let trimmed = input.trim();
    let is_ean13 = trimmed.len() == 13 && trimmed.bytes().all(|b| b.is_ascii_digit());
    match trimmed.strip_prefix('0') {
        Some(rest) if is_ean13 => rest.to_owned(),
        _ => trimmed.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_digit_matches_upc_a_and_legacy_algorithm() {
        assert_eq!(upc_a_check_digit("03600029145"), Some(2));
        assert_eq!(upc_a_check_digit("12345678901"), Some(2));
        assert_eq!(upc_a_check_digit("0360002914"), None);
        assert_eq!(upc_a_check_digit("0360002914X"), None);
        assert!(is_valid_upc_a("036000291452"));
        assert!(!is_valid_upc_a("036000291453"));
    }

    #[test]
    fn order_barcode_is_prefix_2_plus_padded_id() {
        assert_eq!(order_barcode(10_483).as_deref(), Some("200000104832"));
        assert_eq!(order_barcode(1).as_deref(), Some("200000000011"));
        assert_eq!(order_barcode(999_999).as_deref(), Some("200009999996"));
        assert_eq!(order_barcode(0), None);
        assert_eq!(order_barcode(10_000_000_000), None);
    }

    #[test]
    fn new_codes_never_match_legacy_codes_below_a_million_orders() {
        for id in [1, 42, 10_483, 99_999, 999_999] {
            let code = order_barcode(id).unwrap();
            let b = code.as_bytes();
            // Не код по дате: «месяц» (цифры 2–3) равен 00.
            assert_eq!(&code[2..4], "00", "{code}");
            // Не случайный код: первая цифра второго числа (цифра 4) равна 0, а там всегда 1..9.
            assert_eq!(b[4], b'0', "{code}");
        }
    }

    #[test]
    fn normalize_scanned_strips_ean13_leading_zero() {
        assert_eq!(normalize_scanned(" 0200000104832\r\n"), "200000104832");
        assert_eq!(normalize_scanned("200000104832"), "200000104832");
        assert_eq!(normalize_scanned("1234567890123"), "1234567890123");
    }
}
