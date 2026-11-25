//! Запись таблиц в книгу Excel: шапка, итог, форматы денег и дат, ширина колонок.

use chrono::{Datelike, NaiveDate, NaiveDateTime, Timelike};
use mw_domain::money::Money;
use rust_xlsxwriter::{ExcelDateTime, Format, Workbook, Worksheet, XlsxError};

use crate::CoreError;

/// Самая длинная строка в ячейке Excel, знаков.
const CELL_CHARS_MAX: usize = 32_767;
/// Пределы ширины колонки, знаков.
const WIDTH_MIN: usize = 8;
const WIDTH_MAX: usize = 60;

/// Колонка таблицы.
pub(crate) struct Column {
    pub title: String,
}

impl Column {
    pub(crate) fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
        }
    }
}

/// Значение ячейки.
#[derive(Debug, Clone)]
pub(crate) enum Cell {
    Empty,
    Text(String),
    Int(i64),
    Money(Money),
    /// Дата без времени; её понадобятся выгрузкам списков (отчёт пока пишет `Time`).
    #[allow(dead_code)]
    Day(NaiveDate),
    Time(NaiveDateTime),
}

/// Лист: шапка, строки и, если нужно, строка итога под ними.
pub(crate) struct Sheet {
    pub name: String,
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<Cell>>,
    pub total: Option<Vec<Cell>>,
}

fn failed(error: XlsxError) -> CoreError {
    CoreError::Internal(format!("не удалось построить файл Excel: {error}"))
}

/// Форматы книги: обычные и жирные (для шапки и итога).
struct Formats {
    plain: Format,
    bold: Format,
    money: Format,
    money_bold: Format,
    day: Format,
    day_bold: Format,
    time: Format,
    time_bold: Format,
}

impl Formats {
    fn new(currency: &str) -> Self {
        // Внутри кавычек формата своя кавычка не нужна: знак валюты — короткий текст.
        let symbol: String = currency.chars().filter(|c| *c != '"').collect();
        let money = format!("#,##0.00 \"{symbol}\"");
        let bold = |f: Format| f.set_bold();
        let money_f = Format::new().set_num_format(money);
        let day_f = Format::new().set_num_format("dd.mm.yyyy");
        let time_f = Format::new().set_num_format("dd.mm.yyyy hh:mm");
        Self {
            plain: Format::new(),
            bold: Format::new().set_bold(),
            money_bold: bold(money_f.clone()),
            money: money_f,
            day_bold: bold(day_f.clone()),
            day: day_f,
            time_bold: bold(time_f.clone()),
            time: time_f,
        }
    }
}

fn excel_date(date: NaiveDate) -> Option<ExcelDateTime> {
    ExcelDateTime::from_ymd(
        u16::try_from(date.year()).ok()?,
        u8::try_from(date.month()).ok()?,
        u8::try_from(date.day()).ok()?,
    )
    .ok()
}

fn excel_time(at: NaiveDateTime) -> Option<ExcelDateTime> {
    excel_date(at.date())?
        .and_hms(
            u16::try_from(at.hour()).ok()?,
            u8::try_from(at.minute()).ok()?,
            f64::from(at.second()),
        )
        .ok()
}

fn clip(text: &str) -> &str {
    match text.char_indices().nth(CELL_CHARS_MAX) {
        Some((end, _)) => &text[..end],
        None => text,
    }
}

/// Рубли для ячейки: копейки делятся на 100 только при записи файла.
fn rubles(money: Money) -> f64 {
    money.kopecks() as f64 / 100.0
}

/// Сколько знаков занимает значение при показе.
fn shown_chars(cell: &Cell) -> usize {
    match cell {
        Cell::Empty => 0,
        Cell::Text(t) => t.lines().map(|l| l.chars().count()).max().unwrap_or(0),
        Cell::Int(i) => i.to_string().len(),
        Cell::Money(m) => format!("{:.2}", rubles(*m)).len() + 4,
        Cell::Day(_) => 10,
        Cell::Time(_) => 16,
    }
}

fn write_cell<'a>(
    sheet: &mut Worksheet,
    row: u32,
    col: u16,
    cell: &Cell,
    formats: &'a Formats,
    bold: bool,
) -> Result<(), XlsxError> {
    let pick = |plain: &'a Format, strong: &'a Format| if bold { strong } else { plain };
    match cell {
        Cell::Empty => {}
        Cell::Text(text) => {
            // Всегда строка, а не формула: текст «=…» из заказа не исполняется.
            sheet.write_string_with_format(
                row,
                col,
                clip(text),
                pick(&formats.plain, &formats.bold),
            )?;
        }
        Cell::Int(value) => {
            sheet.write_number_with_format(
                row,
                col,
                *value as f64,
                pick(&formats.plain, &formats.bold),
            )?;
        }
        Cell::Money(money) => {
            sheet.write_number_with_format(
                row,
                col,
                rubles(*money),
                pick(&formats.money, &formats.money_bold),
            )?;
        }
        Cell::Day(date) => match excel_date(*date) {
            Some(value) => {
                sheet.write_datetime_with_format(
                    row,
                    col,
                    &value,
                    pick(&formats.day, &formats.day_bold),
                )?;
            }
            // Дата вне диапазона Excel остаётся текстом, а не роняет выгрузку.
            None => {
                sheet.write_string(row, col, date.format("%d.%m.%Y").to_string())?;
            }
        },
        Cell::Time(at) => match excel_time(*at) {
            Some(value) => {
                sheet.write_datetime_with_format(
                    row,
                    col,
                    &value,
                    pick(&formats.time, &formats.time_bold),
                )?;
            }
            None => {
                sheet.write_string(row, col, at.format("%d.%m.%Y %H:%M").to_string())?;
            }
        },
    }
    Ok(())
}

fn write_sheet(sheet: &Sheet, book: &mut Workbook, formats: &Formats) -> Result<(), XlsxError> {
    let columns = sheet.columns.len();
    let width = u16::try_from(columns).unwrap_or(u16::MAX);
    let ws = book.add_worksheet();
    ws.set_name(&sheet.name)?;
    let mut widths: Vec<usize> = sheet
        .columns
        .iter()
        .map(|c| c.title.chars().count())
        .collect();
    for (col, column) in (0u16..).zip(&sheet.columns) {
        ws.write_string_with_format(0, col, &column.title, &formats.bold)?;
    }
    let last_data_row = u32::try_from(sheet.rows.len()).unwrap_or(u32::MAX);
    let total_row = last_data_row.saturating_add(1);
    let all = sheet
        .rows
        .iter()
        .zip(1u32..)
        .map(|(cells, row)| (cells, row, false))
        .chain(sheet.total.iter().map(|cells| (cells, total_row, true)));
    for (cells, row, bold) in all {
        for ((col, cell), size) in (0u16..).zip(cells).zip(widths.iter_mut()) {
            *size = (*size).max(shown_chars(cell));
            write_cell(ws, row, col, cell, formats, bold)?;
        }
    }
    for (col, size) in (0u16..).zip(&widths) {
        ws.set_column_width(col, (*size + 2).clamp(WIDTH_MIN, WIDTH_MAX) as f64)?;
    }
    ws.set_freeze_panes(1, 0)?;
    if width > 0 {
        ws.autofilter(0, 0, last_data_row, width - 1)?;
    }
    Ok(())
}

/// Книга Excel из листов; `currency` — знак валюты для денежных ячеек.
pub(crate) fn workbook(sheets: &[Sheet], currency: &str) -> Result<Vec<u8>, CoreError> {
    let formats = Formats::new(currency);
    let mut book = Workbook::new();
    for sheet in sheets {
        write_sheet(sheet, &mut book, &formats).map_err(failed)?;
    }
    book.save_to_buffer().map_err(failed)
}
