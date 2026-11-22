//! Печать в Windows через GDI: страницы PNG от сервера (растр в разрешении принтера) — в физический размер листа.
//!
//! Чистые расчёты (место страницы на листе, границы, строки DIB) — здесь и под тестами на всех системах; вызовы
//! winspool и GDI — в модуле `gdi`, только в Windows. Каждый блок `unsafe` там — с обоснованием `SAFETY:`, каждый
//! дескриптор (контекст принтера, задание) освобождает страж при `drop`, и при ошибке тоже.

use super::{MAX_DOCUMENT_BYTES, decode_base64};
use std::ops::RangeInclusive;

#[cfg(windows)]
pub(super) use gdi::{print_pages, printers};

/// Разрешение страниц — те же границы, что у растра документа на сервере (`mw-core`).
const DPI: RangeInclusive<u32> = 72..=600;
/// Сторона листа, мм: от маленькой наклейки до A3 с запасом.
const SHEET_MM: RangeInclusive<f32> = 15.0..=500.0;
/// Запас на округление размера страницы при растре, точек.
const SLACK_PX: u32 = 2;
const MM_PER_INCH: f64 = 25.4;

/// Где на листе рисуется страница, в точках принтера: `(x, y, ширина, высота)`. Начало координат контекста
/// принтера — угол области печати, а она отстоит от края листа на `PHYSICALOFFSETX/Y` (`offset_x/y`): страница
/// сдвигается на них назад, чтобы её угол пришёлся на угол листа. Размер — физический: мм → дюймы → точки
/// принтера по `LOGPIXELSX/Y` (`dpi_x/y`). Что за краем области печати, GDI обрезает.
pub(super) fn target_rect(
    width_mm: f32,
    height_mm: f32,
    dpi_x: i32,
    dpi_y: i32,
    offset_x: i32,
    offset_y: i32,
) -> (i32, i32, i32, i32) {
    let dots = |mm: f32, dpi: i32| (f64::from(mm) * f64::from(dpi) / MM_PER_INCH).round() as i32;
    (
        -offset_x,
        -offset_y,
        dots(width_mm, dpi_x),
        dots(height_mm, dpi_y),
    )
}

/// Лист и разрешение в границах; наибольший размер картинки страницы в точках — лист в `dpi` с запасом на
/// округление. Больше — не страница этого листа; заодно это предел памяти: картинку больше (по заголовку PNG)
/// декодер не раскрывает.
pub(super) fn page_limits(width_mm: f32, height_mm: f32, dpi: u32) -> Result<(u32, u32), String> {
    if !(SHEET_MM.contains(&width_mm) && SHEET_MM.contains(&height_mm)) {
        return Err(format!(
            "Размер листа должен быть от {} до {} мм",
            SHEET_MM.start(),
            SHEET_MM.end()
        ));
    }
    if !DPI.contains(&dpi) {
        return Err(format!(
            "Разрешение должно быть от {} до {} точек на дюйм",
            DPI.start(),
            DPI.end()
        ));
    }
    let pixels = |mm: f32| (f64::from(mm) * f64::from(dpi) / MM_PER_INCH).ceil() as u32 + SLACK_PX;
    Ok((pixels(width_mm), pixels(height_mm)))
}

/// Страницы PNG из base64: все вместе не больше [`MAX_DOCUMENT_BYTES`].
pub(super) fn decode_pages(pages: &[String]) -> Result<Vec<Vec<u8>>, String> {
    if pages.is_empty() {
        return Err("Нет страниц для печати".to_string());
    }
    let mut left = MAX_DOCUMENT_BYTES;
    pages
        .iter()
        .map(|page| {
            let bytes = decode_base64(page, left)?;
            left -= bytes.len();
            Ok(bytes)
        })
        .collect()
}

/// Текст ошибки шага с текстом Windows, если она его дала. GDI ставит код ошибки не всегда: без него (код 0) —
/// только `what`, неизменный текст, который называет шаг.
pub(super) fn with_windows_text(what: &str, windows: Option<&str>) -> String {
    match windows.map(str::trim) {
        Some(text) if !text.is_empty() => format!("{what}: {text}"),
        _ => what.to_string(),
    }
}

/// Строки 24-битного DIB из RGBA (`width` точек в строке): синий, зелёный, красный; строка дополнена нулями до
/// кратной 4 байтам длины; порядок строк — как в картинке (DIB сверху вниз). Прозрачное ложится на белый лист.
pub(super) fn dib_rows(rgba: &[u8], width: usize) -> Vec<u8> {
    if width == 0 {
        return Vec::new();
    }
    let stride = (width * 3).next_multiple_of(4);
    let rows = rgba.len() / (width * 4);
    let mut out = Vec::with_capacity(stride * rows);
    let on_white = |channel: u8, alpha: u8| {
        let (channel, alpha) = (u32::from(channel), u32::from(alpha));
        ((channel * alpha + 255 * (255 - alpha) + 127) / 255) as u8
    };
    for row in rgba.chunks_exact(width * 4) {
        for pixel in row.chunks_exact(4) {
            let alpha = pixel[3];
            out.extend([pixel[2], pixel[1], pixel[0]].map(|channel| on_white(channel, alpha)));
        }
        out.resize(out.len() + stride - width * 3, 0);
    }
    out
}

#[cfg(windows)]
#[allow(
    unsafe_code,
    reason = "вызовы winspool и GDI; у каждого блока — обоснование SAFETY"
)]
#[deny(clippy::undocumented_unsafe_blocks)]
mod gdi {
    use super::super::{PrinterInfo, check_copies, known_printer};
    use super::{decode_pages, dib_rows, page_limits, target_rect, with_windows_text};
    use ::windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, SetLastError, WIN32_ERROR};
    use ::windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, COLORONCOLOR, CreateDCW, DIB_RGB_COLORS, DeleteDC,
        GDI_ERROR, GET_DEVICE_CAPS_INDEX, GetDeviceCaps, HDC, LOGPIXELSX, LOGPIXELSY,
        PHYSICALOFFSETX, PHYSICALOFFSETY, SRCCOPY, SetStretchBltMode, StretchDIBits,
    };
    use ::windows::Win32::Graphics::Printing::{
        EnumPrintersW, GetDefaultPrinterW, PRINTER_ENUM_CONNECTIONS, PRINTER_ENUM_LOCAL,
        PRINTER_INFO_4W,
    };
    use ::windows::Win32::Storage::Xps::{
        AbortDoc, DOCINFOW, EndDoc, EndPage, StartDocW, StartPage,
    };
    use ::windows::core::{Error, PCWSTR, PWSTR, w};
    use std::io::Cursor;
    use std::mem::{size_of, size_of_val};

    /// Название задания в очереди печати.
    const JOB_TITLE: &str = "MyWork";
    /// Сколько раз перечисление принтеров пробует буфер: между вызовами принтеры могут добавиться.
    const ENUM_TRIES: usize = 4;

    /// Принтеры этого компьютера: свои и подключённые сетевые.
    pub(crate) fn printers() -> Result<Vec<PrinterInfo>, String> {
        let names = printer_names()?;
        let default = default_printer();
        Ok(names
            .into_iter()
            .map(|name| PrinterInfo {
                is_default: default.as_deref() == Some(name.as_str()),
                name,
            })
            .collect())
    }

    /// Печатает страницы PNG: сначала проверки (копии, лист, страницы, принтер), затем задание GDI. Копии идут
    /// подряд за каждой страницей (как `lp -n` без `collate`): в памяти — одна раскрытая страница.
    pub(crate) fn print_pages(
        printer: &str,
        pages_base64: &[String],
        dpi: u32,
        width_mm: f32,
        height_mm: f32,
        copies: u32,
    ) -> Result<(), String> {
        check_copies(copies)?;
        let max = page_limits(width_mm, height_mm, dpi)?;
        let pages = decode_pages(pages_base64)?;
        // Заголовки всех страниц — до начала задания: испорченная страница не оставит половину документа.
        for page in &pages {
            png_reader(page, max)
                .into_dimensions()
                .map_err(page_error)?;
        }
        known_printer(printer, &printers()?)?;

        let dc = PrinterDc::open(printer)?;
        let (dpi_x, dpi_y) = (dc.caps(LOGPIXELSX), dc.caps(LOGPIXELSY));
        if dpi_x <= 0 || dpi_y <= 0 {
            return Err("Ошибка печати: принтер не сообщил своё разрешение".to_string());
        }
        let rect = target_rect(
            width_mm,
            height_mm,
            dpi_x,
            dpi_y,
            dc.caps(PHYSICALOFFSETX),
            dc.caps(PHYSICALOFFSETY),
        );
        dc.sharp_stretching();
        // Задание — после контекста: при выходе (и при ошибке) сначала `AbortDoc`, потом `DeleteDC`.
        let job = PrintJob::start(&dc)?;
        for page in &pages {
            let dib = Dib::decode(page, max)?;
            for _ in 0..copies {
                job.page(&dib, rect)?;
            }
        }
        job.finish()
    }

    /// Имена принтеров (`EnumPrintersW`, уровень 4 — только имена, без опроса принтеров).
    fn printer_names() -> Result<Vec<String>, String> {
        let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
        // Слова u64: буфер выровнен для `PRINTER_INFO_4W` (в ней указатели) и строк UTF-16 за ней.
        let mut words: Vec<u64> = Vec::new();
        for _ in 0..ENUM_TRIES {
            let (mut needed, mut returned) = (0u32, 0u32);
            let bytes = as_bytes_mut(&mut words);
            let buffer = if bytes.is_empty() { None } else { Some(bytes) };
            clear_last_error();
            // SAFETY: `buffer` — живой срез (или его нет), его длину функция получает вместе с ним; `needed` и
            // `returned` — указатели на живые локальные переменные; имя сервера — пустое (этот компьютер).
            let listed = unsafe {
                EnumPrintersW(flags, PCWSTR::null(), 4, buffer, &mut needed, &mut returned)
            };
            match listed {
                Ok(()) => return Ok(read_names(&words, returned)),
                Err(e)
                    if e.code() == ERROR_INSUFFICIENT_BUFFER.to_hresult()
                        && needed as usize > size_of_val(words.as_slice()) =>
                {
                    words = vec![0; (needed as usize).div_ceil(size_of::<u64>())];
                }
                Err(e) => {
                    return Err(with_windows_text(
                        "Не удалось получить список принтеров",
                        windows_text(&e).as_deref(),
                    ));
                }
            }
        }
        Err("Не удалось получить список принтеров: список менялся во время чтения".to_string())
    }

    /// Те же слова как байты — буфер для `EnumPrintersW`.
    fn as_bytes_mut(words: &mut [u64]) -> &mut [u8] {
        let len = size_of_val(words);
        // SAFETY: у u8 нет недопустимых значений и выравнивание 1; срез покрывает ровно память тех же слов и
        // заимствует их исключительно на то же время, что и `words`.
        unsafe { std::slice::from_raw_parts_mut(words.as_mut_ptr().cast::<u8>(), len) }
    }

    /// Имена из буфера `EnumPrintersW`: в начале — `count` структур `PRINTER_INFO_4W`, их строки — дальше в нём же.
    fn read_names(words: &[u64], count: u32) -> Vec<String> {
        let fits = size_of_val(words) / size_of::<PRINTER_INFO_4W>();
        let infos = words.as_ptr().cast::<PRINTER_INFO_4W>();
        (0..(count as usize).min(fits))
            .filter_map(|i| {
                // SAFETY: `i` < `fits`: структура целиком внутри буфера; буфер выровнен на 8 (слова u64) — как и
                // `PRINTER_INFO_4W`; её записала `EnumPrintersW`.
                let info = unsafe { infos.add(i).read() };
                if info.pPrinterName.is_null() {
                    return None;
                }
                // SAFETY: `pPrinterName` указывает на строку с нулём на конце в том же буфере, а он жив (`words`).
                unsafe { info.pPrinterName.to_string() }.ok()
            })
            .collect()
    }

    /// Принтер по умолчанию; нет его — `None`.
    fn default_printer() -> Option<String> {
        let mut len = 0u32;
        // SAFETY: без буфера функция только пишет нужную длину (в символах, с нулём) в живую переменную `len`.
        let _ = unsafe { GetDefaultPrinterW(None, &mut len) };
        if len == 0 {
            return None;
        }
        let mut name = vec![0u16; len as usize];
        // SAFETY: буфер `name` на `len` символов жив до конца вызова; функция пишет не больше `len` символов с нулём.
        let found = unsafe { GetDefaultPrinterW(Some(PWSTR(name.as_mut_ptr())), &mut len) };
        if !found.as_bool() {
            return None;
        }
        let end = name.iter().position(|&c| c == 0).unwrap_or(name.len());
        String::from_utf16(&name[..end]).ok()
    }

    /// Сбрасывает код последней ошибки потока перед проверяемым вызовом: GDI ставит его не всегда, и без сброса
    /// в тексте ошибки оказался бы код прошлого вызова.
    fn clear_last_error() {
        // SAFETY: функция только записывает код последней ошибки текущего потока.
        unsafe { SetLastError(WIN32_ERROR(0)) };
    }

    /// Текст ошибки Windows; для кода 0 (ошибки нет) — `None`.
    fn windows_text(error: &Error) -> Option<String> {
        (!error.code().is_ok()).then(|| error.message())
    }

    /// Ошибка шага печати с текстом последней ошибки Windows (если она есть); вызывать сразу после шага, перед
    /// которым был [`clear_last_error`].
    fn print_error(step: &str) -> String {
        with_windows_text(
            &format!("Ошибка печати: {step}"),
            windows_text(&Error::from_thread()).as_deref(),
        )
    }

    fn page_error(error: image::ImageError) -> String {
        match error {
            image::ImageError::Limits(_) => "Страница для печати больше листа".to_string(),
            _ => "Страница для печати повреждена".to_string(),
        }
    }

    /// Чтение PNG не больше `max` точек по сторонам.
    fn png_reader(png: &[u8], max: (u32, u32)) -> image::ImageReader<Cursor<&[u8]>> {
        let mut reader = image::ImageReader::with_format(Cursor::new(png), image::ImageFormat::Png);
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(max.0);
        limits.max_image_height = Some(max.1);
        reader.limits(limits);
        reader
    }

    /// Контекст принтера; `DeleteDC` — при `drop`, на любом пути.
    struct PrinterDc(HDC);

    impl PrinterDc {
        fn open(printer: &str) -> Result<Self, String> {
            let name = wide(printer);
            clear_last_error();
            // SAFETY: имя драйвера — литерал с нулём, имя принтера — живой буфер с нулём (`wide`); порт и DEVMODE
            // не передаются — настройки принтера по умолчанию.
            let dc =
                unsafe { CreateDCW(w!("WINSPOOL"), PCWSTR(name.as_ptr()), PCWSTR::null(), None) };
            if dc.is_invalid() {
                Err(print_error("принтер не открылся"))
            } else {
                Ok(Self(dc))
            }
        }

        fn caps(&self, index: GET_DEVICE_CAPS_INDEX) -> i32 {
            // SAFETY: контекст открыт и принадлежит `self` до `drop`.
            unsafe { GetDeviceCaps(Some(self.0), index) }
        }

        /// Растяжение без сглаживания: штрихкод остаётся чётким.
        fn sharp_stretching(&self) {
            // SAFETY: контекст открыт и принадлежит `self` до `drop`.
            let _ = unsafe { SetStretchBltMode(self.0, COLORONCOLOR) };
        }
    }

    impl Drop for PrinterDc {
        fn drop(&mut self) {
            // SAFETY: контекст создан `CreateDCW`, удаляется один раз — здесь; задание на нём к этому времени
            // закончено или отменено (`PrintJob` заимствует контекст и выходит раньше).
            let _ = unsafe { DeleteDC(self.0) };
        }
    }

    /// Задание печати; без успешного `finish` — `AbortDoc` при `drop`: спулер выбрасывает начатое.
    struct PrintJob<'dc> {
        dc: &'dc PrinterDc,
        open: bool,
    }

    impl<'dc> PrintJob<'dc> {
        fn start(dc: &'dc PrinterDc) -> Result<Self, String> {
            let title = wide(JOB_TITLE);
            let info = DOCINFOW {
                cbSize: size_of::<DOCINFOW>() as i32,
                lpszDocName: PCWSTR(title.as_ptr()),
                ..Default::default()
            };
            clear_last_error();
            // SAFETY: `info` и строка названия живы до конца вызова; контекст открыт.
            if unsafe { StartDocW(dc.0, &info) } <= 0 {
                return Err(print_error("задание не началось"));
            }
            Ok(Self { dc, open: true })
        }

        /// Одна страница листа: `dib` растягивается в `rect` (см. [`target_rect`]).
        fn page(
            &self,
            dib: &Dib,
            (x, y, width, height): (i32, i32, i32, i32),
        ) -> Result<(), String> {
            clear_last_error();
            // SAFETY: контекст открыт, задание начато (`open`).
            if unsafe { StartPage(self.dc.0) } <= 0 {
                return Err(print_error("страница не началась"));
            }
            clear_last_error();
            // SAFETY: `dib.bits` — строки картинки `dib.width` × `dib.height`, как описывает `dib.info` (24 бита,
            // строка кратна 4 байтам); оба живы до конца вызова, функция их только читает. Начатую страницу при
            // ошибке выбрасывает `AbortDoc` (`drop` задания).
            let lines = unsafe {
                StretchDIBits(
                    self.dc.0,
                    x,
                    y,
                    width,
                    height,
                    0,
                    0,
                    dib.width,
                    dib.height,
                    Some(dib.bits.as_ptr().cast()),
                    &dib.info,
                    DIB_RGB_COLORS,
                    SRCCOPY,
                )
            };
            if lines == 0 || lines == GDI_ERROR {
                return Err(print_error("страница не нарисовалась"));
            }
            clear_last_error();
            // SAFETY: страница начата выше.
            if unsafe { EndPage(self.dc.0) } <= 0 {
                return Err(print_error("страница не закончилась"));
            }
            Ok(())
        }

        fn finish(mut self) -> Result<(), String> {
            clear_last_error();
            // SAFETY: задание начато и ещё не закончено (`open`).
            if unsafe { EndDoc(self.dc.0) } <= 0 {
                // `drop` отменит задание.
                return Err(print_error("задание не закончилось"));
            }
            self.open = false;
            Ok(())
        }
    }

    impl Drop for PrintJob<'_> {
        fn drop(&mut self) {
            if self.open {
                // SAFETY: задание на открытом контексте начато и не закончено; отменяется один раз — здесь.
                let _ = unsafe { AbortDoc(self.dc.0) };
            }
        }
    }

    /// Раскрытая страница: 24-битный DIB сверху вниз.
    struct Dib {
        info: BITMAPINFO,
        bits: Vec<u8>,
        width: i32,
        height: i32,
    }

    impl Dib {
        fn decode(png: &[u8], max: (u32, u32)) -> Result<Self, String> {
            let image = png_reader(png, max)
                .decode()
                .map_err(page_error)?
                .into_rgba8();
            let (width, height) = image.dimensions();
            // Размеры не больше `max` (`page_limits`: тысячи точек) — в i32 помещаются.
            let (width, height) = (width as i32, height as i32);
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    // Отрицательная высота — строки сверху вниз, как в PNG.
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 24,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            Ok(Self {
                info,
                bits: dib_rows(image.as_raw(), width as usize),
                width,
                height,
            })
        }
    }

    /// Строка UTF-16 с нулём на конце.
    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(Some(0)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD;

    /// Наклейка 58×40 на термопринтере 203 dpi и A4 на лазерном 600 dpi: размер — физический (мм → дюймы →
    /// точки принтера, с округлением), начало — на краю листа, то есть на поля области печати назад.
    #[test]
    fn a_page_fills_the_physical_sheet_from_its_edge() {
        // 58 мм × 203 / 25,4 = 463,5 → 464; 40 мм → 319,7 → 320.
        assert_eq!(target_rect(58.0, 40.0, 203, 203, 0, 0), (0, 0, 464, 320));
        assert_eq!(
            target_rect(58.0, 40.0, 203, 203, 12, 8),
            (-12, -8, 464, 320)
        );
        // 210 мм × 600 / 25,4 = 4960,6 → 4961; 297 мм → 7015,7 → 7016; поля 4,23 мм = 100 точек.
        assert_eq!(
            target_rect(210.0, 297.0, 600, 600, 100, 100),
            (-100, -100, 4961, 7016)
        );
        // Разное разрешение по осям: 600 × 300 dpi.
        assert_eq!(
            target_rect(210.0, 297.0, 600, 300, 100, 50),
            (-100, -50, 4961, 3508)
        );
    }

    #[test]
    fn a_page_is_at_most_its_sheet_in_pixels() {
        // Сервер растрирует с округлением: запас — две точки.
        assert_eq!(page_limits(58.0, 40.0, 203), Ok((466, 322)));
        assert_eq!(page_limits(210.0, 297.0, 600), Ok((4963, 7018)));
        assert_eq!(page_limits(15.0, 500.0, 72), Ok((45, 1420)));
        for (w, h) in [(14.9, 40.0), (58.0, 500.1), (f32::NAN, 40.0), (58.0, -1.0)] {
            assert_eq!(
                page_limits(w, h, 203),
                Err("Размер листа должен быть от 15 до 500 мм".to_string()),
                "{w}×{h}"
            );
        }
        for dpi in [0, 71, 601] {
            assert_eq!(
                page_limits(58.0, 40.0, dpi),
                Err("Разрешение должно быть от 72 до 600 точек на дюйм".to_string()),
                "{dpi}"
            );
        }
    }

    #[test]
    fn pages_share_the_document_limit() {
        let pages = vec![STANDARD.encode(b"page 1"), STANDARD.encode(b"page 2")];
        assert_eq!(
            decode_pages(&pages),
            Ok(vec![b"page 1".to_vec(), b"page 2".to_vec()])
        );
        assert_eq!(decode_pages(&[]), Err("Нет страниц для печати".to_string()));
        assert_eq!(
            decode_pages(&[STANDARD.encode(b"page"), "%%%".to_string()]),
            Err("Документ для печати повреждён".to_string())
        );
        // Каждая страница меньше предела, вместе — больше.
        let half = STANDARD.encode(vec![0u8; super::super::MAX_DOCUMENT_BYTES / 2 + 1]);
        assert_eq!(
            decode_pages(&[half.clone(), half]),
            Err("Документ для печати больше 64 МБ".to_string())
        );
    }

    /// GDI ставит код ошибки не всегда: без текста Windows (или с пустым) — только шаг, как он есть.
    #[test]
    fn a_failed_step_names_itself_and_adds_the_windows_text_when_there_is_one() {
        let step = "Ошибка печати: страница не началась";
        assert_eq!(with_windows_text(step, None), step);
        assert_eq!(with_windows_text(step, Some(" \r\n")), step);
        assert_eq!(
            with_windows_text(step, Some("Отказано в доступе.\r\n")),
            "Ошибка печати: страница не началась: Отказано в доступе."
        );
    }

    /// DIB: синий, зелёный, красный; строка выровнена на 4 байта; прозрачное — на белом (лист белый).
    #[test]
    fn rgba_becomes_dib_rows_on_white() {
        let rgba = [
            255, 0, 0, 255, // красный
            0, 0, 255, 0, // прозрачный синий — белый
            0, 0, 0, 128, // полупрозрачный чёрный — серый
            10, 20, 30, 255, //
        ];
        // Две строки по две точки: 6 байт + 2 выравнивания.
        assert_eq!(
            dib_rows(&rgba, 2),
            [
                0, 0, 255, 255, 255, 255, 0, 0, //
                127, 127, 127, 30, 20, 10, 0, 0,
            ]
        );
        // Одна точка в строке: 3 байта + 1.
        assert_eq!(dib_rows(&[1, 2, 3, 255], 1), [3, 2, 1, 0]);
        // Четыре точки — 12 байт, выравнивание не нужно.
        assert_eq!(dib_rows(&[0; 16], 4).len(), 12);
        assert!(dib_rows(&[], 0).is_empty());
    }
}
