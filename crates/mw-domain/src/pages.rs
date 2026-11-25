//! Выбор страниц документа для печати: «1,3-4» — первая, третья и четвёртая страницы.

use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};

/// Страницы документа: номера с 1, по возрастанию, без повторов, хотя бы одна.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<u16>", into = "Vec<u16>")]
pub struct PageSelection(Vec<u16>);

const FORMAT: &str = "Страницы — номера и диапазоны через запятую, например 1, 3-4";
const ZERO: &str = "Страницы нумеруются с 1";
const BACKWARDS: &str = "Диапазон страниц идёт от меньшего номера к большему, например 3-4";
const TOO_LARGE: &str = "Слишком большой номер страницы";

impl PageSelection {
    /// Разбирает выбор вида «1,3-4»: номера и диапазоны через запятую, вокруг них — пробелы,
    /// в диапазоне — «-» или «–». Ошибка — сообщение для человека.
    ///
    /// Диапазоны сливаются до того, как развернуться в номера: сколько бы их ни было, номеров не
    /// больше 65 535.
    pub fn parse(text: &str) -> Result<Self, &'static str> {
        let mut ranges = text.split(',').map(range).collect::<Result<Vec<_>, _>>()?;
        ranges.sort_unstable_by_key(|r| *r.start());
        let mut merged: Vec<RangeInclusive<u16>> = Vec::with_capacity(ranges.len());
        for r in ranges {
            match merged.last_mut() {
                Some(last) if u32::from(*r.start()) <= u32::from(*last.end()) + 1 => {
                    *last = *last.start()..=(*last.end()).max(*r.end());
                }
                _ => merged.push(r),
            }
        }
        Ok(Self(merged.into_iter().flatten().collect()))
    }

    /// Выбрана ли страница `page` (номер с 1).
    pub fn contains(&self, page: usize) -> bool {
        u16::try_from(page).is_ok_and(|page| self.0.binary_search(&page).is_ok())
    }

    /// Номера страниц по возрастанию.
    pub fn as_slice(&self) -> &[u16] {
        &self.0
    }
}

/// Номер «3» или диапазон «3-4» / «3–4», с пробелами вокруг.
fn range(part: &str) -> Result<RangeInclusive<u16>, &'static str> {
    let (first, last) = match part.split_once(['-', '–']) {
        Some((first, last)) => (number(first)?, number(last)?),
        None => {
            let page = number(part)?;
            (page, page)
        }
    };
    if first > last {
        return Err(BACKWARDS);
    }
    Ok(first..=last)
}

/// Номер страницы: цифры, вокруг — пробелы; не 0 и не больше 65 535.
fn number(text: &str) -> Result<u16, &'static str> {
    let digits = text.trim();
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(FORMAT);
    }
    match digits.parse::<u16>() {
        Ok(0) => Err(ZERO),
        Ok(page) => Ok(page),
        Err(_) => Err(TOO_LARGE),
    }
}

impl TryFrom<Vec<u16>> for PageSelection {
    type Error = &'static str;

    /// Номера в любом порядке, с повторами; пусто или 0 — ошибка.
    fn try_from(mut pages: Vec<u16>) -> Result<Self, Self::Error> {
        if pages.is_empty() {
            return Err(FORMAT);
        }
        if pages.contains(&0) {
            return Err(ZERO);
        }
        pages.sort_unstable();
        pages.dedup();
        Ok(Self(pages))
    }
}

impl From<PageSelection> for Vec<u16> {
    fn from(selection: PageSelection) -> Self {
        selection.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pages(text: &str) -> Result<Vec<u16>, &'static str> {
        PageSelection::parse(text).map(|s| s.as_slice().to_vec())
    }

    #[test]
    fn numbers_and_ranges_become_sorted_distinct_pages() {
        assert_eq!(pages("1"), Ok(vec![1]));
        assert_eq!(pages("1,3"), Ok(vec![1, 3]));
        assert_eq!(pages(" 1, 3-4 "), Ok(vec![1, 3, 4]));
        assert_eq!(pages("3–4"), Ok(vec![3, 4]), "en dash");
        assert_eq!(pages("4,1,1"), Ok(vec![1, 4]));
        assert_eq!(pages("2-3,1-4"), Ok(vec![1, 2, 3, 4]), "ranges overlap");
        assert_eq!(pages("5 - 5"), Ok(vec![5]));
    }

    #[test]
    fn empty_zero_backwards_foreign_open_and_too_large_are_refused() {
        for bad in [
            "", "  ", "0", "2-1", "a", "1-", "-2", "1,,2", "1,", "70000", "+1", "1-2-3",
        ] {
            let res = pages(bad);
            assert!(res.is_err(), "{bad:?}: {res:?}");
            assert!(!res.unwrap_err().is_empty(), "{bad:?}");
        }
    }

    /// Огромные диапазоны не раздувают память: повторы сливаются до разворота в номера.
    #[test]
    fn many_wide_ranges_stay_one_set_of_pages() {
        let text = vec!["1-65535"; 2000].join(",");
        let selection = PageSelection::parse(&text).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(selection.as_slice().len(), 65_535);
        assert_eq!(selection.as_slice().last(), Some(&65_535));
    }

    #[test]
    fn contains_counts_from_one() {
        let selection = PageSelection::parse("2,4-5").unwrap_or_else(|e| panic!("{e}"));
        let chosen: Vec<usize> = (0..8).filter(|&p| selection.contains(p)).collect();
        assert_eq!(chosen, [2, 4, 5]);
        assert!(!selection.contains(65_536 + 2), "не обрезается до u16");
    }

    #[test]
    fn json_is_the_list_of_pages_and_a_wrong_list_is_refused() {
        let selection = PageSelection::parse("3,1").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            serde_json::to_string(&selection).ok().as_deref(),
            Some("[1,3]")
        );
        let back: Result<PageSelection, _> = serde_json::from_str("[3,1,3]");
        assert_eq!(back.ok(), Some(selection));
        for bad in ["[]", "[0]", "[70000]"] {
            assert!(serde_json::from_str::<PageSelection>(bad).is_err(), "{bad}");
        }
    }
}
