//! Печать на принтер этого компьютера: команды Tauri для интерфейса.
//!
//! Документ строит сервер, интерфейс передаёт его сюда: на macOS и Linux — PDF в CUPS (`lp`, модуль `cups`), на
//! Windows — страницы PNG в разрешении принтера через GDI (модуль `windows`). Ошибки — строки по-русски: интерфейс
//! показывает их в уведомлении как есть.

#[cfg(not(windows))]
mod cups;
// Чистые расчёты страницы проверяются тестами и не на Windows; вызовы GDI внутри — только `cfg(windows)`.
#[cfg(any(windows, test))]
mod windows;

use serde::Serialize;
use std::ops::RangeInclusive;

/// Принтер этого компьютера.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PrinterInfo {
    /// Имя очереди (CUPS) или принтера (Windows): его же получает печать.
    pub name: String,
    /// Принтер по умолчанию в системе.
    pub is_default: bool,
}

/// Сколько копий можно заказать за раз.
const COPIES: RangeInclusive<u32> = 1..=99;
/// Предел документа (всех страниц вместе) после base64: защита от случайно огромного ввода.
const MAX_DOCUMENT_BYTES: usize = 64 * 1024 * 1024;

/// Принтеры этого компьютера; без принтеров — пустой список.
#[tauri::command]
pub async fn list_printers() -> Result<Vec<PrinterInfo>, String> {
    #[cfg(not(windows))]
    let printers = cups::printers;
    #[cfg(windows)]
    let printers = windows::printers;
    blocking(printers).await
}

/// В каком виде печать ждёт документ: `"png"` — страницы для GDI (Windows), `"pdf"` — файл для CUPS.
#[tauri::command]
pub fn print_format() -> &'static str {
    if cfg!(windows) { "png" } else { "pdf" }
}

/// Печать PDF через CUPS (macOS, Linux). `media_mm` — размер наклейки (ширина, высота) в мм, у актов — `None`.
#[tauri::command]
pub async fn print_pdf(
    printer: String,
    pdf_base64: String,
    copies: u32,
    media_mm: Option<(f32, f32)>,
) -> Result<(), String> {
    #[cfg(not(windows))]
    {
        blocking(move || cups::print_pdf(&printer, &pdf_base64, copies, media_mm)).await
    }
    #[cfg(windows)]
    {
        let _ = (printer, pdf_base64, copies, media_mm);
        Err("В Windows печатаются страницы, а не PDF".to_string())
    }
}

/// Печать страниц PNG через GDI (Windows): страницы `dpi` точек на дюйм, лист `width_mm` × `height_mm`.
#[tauri::command]
pub async fn print_pages(
    printer: String,
    pages_base64: Vec<String>,
    dpi: u32,
    width_mm: f32,
    height_mm: f32,
    copies: u32,
) -> Result<(), String> {
    #[cfg(windows)]
    {
        blocking(move || {
            windows::print_pages(&printer, &pages_base64, dpi, width_mm, height_mm, copies)
        })
        .await
    }
    #[cfg(not(windows))]
    {
        let _ = (printer, pages_base64, dpi, width_mm, height_mm, copies);
        Err("Страницами печатает только Windows, здесь — PDF".to_string())
    }
}

/// Работа с процессами и спулером — вне потоков асинхронной среды: окно и сервер не ждут принтер.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| format!("Ошибка печати: {e}"))?
}

/// Число копий в границах [`COPIES`].
fn check_copies(copies: u32) -> Result<(), String> {
    if COPIES.contains(&copies) {
        Ok(())
    } else {
        Err(format!(
            "Число копий должно быть от {} до {}",
            COPIES.start(),
            COPIES.end()
        ))
    }
}

/// Принтер выбран и есть среди принтеров этого компьютера (имя — точно, как его назвала система).
fn known_printer(printer: &str, printers: &[PrinterInfo]) -> Result<(), String> {
    if printer.is_empty() {
        return Err("Принтер не выбран".to_string());
    }
    if printers.iter().any(|known| known.name == printer) {
        Ok(())
    } else {
        Err(format!("Принтер «{printer}» не найден"))
    }
}

/// Байты из base64 не длиннее `limit`; длина текста проверяется до раскодирования. В тексте ошибки —
/// предел всего документа ([`MAX_DOCUMENT_BYTES`]): `limit` у страниц — его остаток.
fn decode_base64(text: &str, limit: usize) -> Result<Vec<u8>, String> {
    use base64::Engine as _;
    let too_big = || {
        format!(
            "Документ для печати больше {} МБ",
            MAX_DOCUMENT_BYTES / (1024 * 1024)
        )
    };
    if text.is_empty() {
        return Err("Документ для печати пуст".to_string());
    }
    if text.len() > limit.div_ceil(3) * 4 {
        return Err(too_big());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(text)
        .map_err(|_| "Документ для печати повреждён".to_string())?;
    if bytes.len() > limit {
        return Err(too_big());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD;

    fn printer(name: &str, is_default: bool) -> PrinterInfo {
        PrinterInfo {
            name: name.to_string(),
            is_default,
        }
    }

    #[test]
    fn the_print_format_follows_the_system() {
        let expected = if cfg!(windows) { "png" } else { "pdf" };
        assert_eq!(print_format(), expected);
    }

    /// Форма ответа `list_printers` для интерфейса.
    #[test]
    fn a_printer_reaches_the_interface_with_its_name_and_default_flag() {
        assert_eq!(
            serde_json::to_value(printer("Наклейки", true)).unwrap(),
            serde_json::json!({ "name": "Наклейки", "is_default": true })
        );
    }

    #[test]
    fn copies_go_from_one_to_ninety_nine() {
        assert_eq!(check_copies(1), Ok(()));
        assert_eq!(check_copies(99), Ok(()));
        for copies in [0, 100, u32::MAX] {
            assert_eq!(
                check_copies(copies),
                Err("Число копий должно быть от 1 до 99".to_string()),
                "{copies}"
            );
        }
    }

    #[test]
    fn printing_needs_a_chosen_printer_of_this_computer() {
        let printers = [printer("Акты", true), printer("Наклейки_Zebra", false)];
        assert_eq!(known_printer("Наклейки_Zebra", &printers), Ok(()));
        assert_eq!(
            known_printer("", &printers),
            Err("Принтер не выбран".to_string())
        );
        assert_eq!(
            known_printer("Пропавший принтер", &printers),
            Err("Принтер «Пропавший принтер» не найден".to_string())
        );
        // Имя сравнивается точно: похожее — другой принтер.
        assert_eq!(
            known_printer("акты", &printers),
            Err("Принтер «акты» не найден".to_string())
        );
        assert_eq!(
            known_printer("Акты", &[]),
            Err("Принтер «Акты» не найден".to_string())
        );
    }

    #[test]
    fn a_document_is_decoded_within_its_limit() {
        let bytes = b"%PDF-1.7 test".to_vec();
        assert_eq!(decode_base64(&STANDARD.encode(&bytes), 64), Ok(bytes));
        let too_big = Err("Документ для печати больше 64 МБ".to_string());
        // Ровно предел проходит, на байт больше — нет (и до раскодирования, по длине текста).
        assert!(decode_base64(&STANDARD.encode([7u8; 10]), 10).is_ok());
        assert_eq!(decode_base64(&STANDARD.encode([7u8; 11]), 10), too_big);
        assert_eq!(decode_base64(&"A".repeat(1_000_000), 10), too_big);
        assert_eq!(
            decode_base64("это не base64", 64),
            Err("Документ для печати повреждён".to_string())
        );
        assert_eq!(
            decode_base64("", 64),
            Err("Документ для печати пуст".to_string())
        );
    }
}
