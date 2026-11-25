//! Коды мест на складе: шкаф, полка, ячейка. Чистые функции без ввода-вывода.

use crate::search::qwerty_from_jcuken;
use crate::validation::FieldError;
use std::cmp::Ordering;

pub const MAX_RACK_SHELVES: i32 = 20;
pub const MAX_RACK_CELLS_PER_SHELF: i32 = 20;
pub const MAX_RACK_CELLS: i32 = 100;
pub const MAX_RACK_CODE_CHARS: usize = 8;
pub const MAX_RACK_LABEL_CHARS: usize = 100;

/// Код шкафа: без пробелов по краям, заглавные латинские, 1–8 знаков `[A-Z0-9]`, хотя бы одна буква.
pub fn normalize_rack_code(raw: &str) -> Option<String> {
    let code = raw.trim().to_ascii_uppercase();
    let valid = (1..=MAX_RACK_CODE_CHARS).contains(&code.len())
        && code
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        && code.bytes().any(|b| b.is_ascii_uppercase());
    valid.then_some(code)
}

/// Сетка шкафа: полки и ячейки на каждой полке.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grid {
    pub shelves: i32,
    pub cells_per_shelf: i32,
}

impl Grid {
    pub fn validate(self) -> Vec<FieldError> {
        let mut errors = Vec::new();
        let shelves_ok = (1..=MAX_RACK_SHELVES).contains(&self.shelves);
        let cells_ok = (1..=MAX_RACK_CELLS_PER_SHELF).contains(&self.cells_per_shelf);
        if !shelves_ok {
            errors.push(FieldError::new("shelves", "out_of_range"));
        }
        if !cells_ok {
            errors.push(FieldError::new("cells_per_shelf", "out_of_range"));
        }
        if shelves_ok && cells_ok && self.shelves * self.cells_per_shelf > MAX_RACK_CELLS {
            errors.push(FieldError::new("cells_per_shelf", "rack_too_big"));
        }
        errors
    }

    pub fn contains(self, shelf: i32, cell: i32) -> bool {
        (1..=self.shelves).contains(&shelf) && (1..=self.cells_per_shelf).contains(&cell)
    }

    /// Все ячейки: полка за полкой, ячейка за ячейкой.
    pub fn cells(self) -> impl Iterator<Item = (i32, i32)> {
        (1..=self.shelves).flat_map(move |s| (1..=self.cells_per_shelf).map(move |c| (s, c)))
    }
}

/// Код ячейки: `B-2-4`; ось из одного значения в код не входит.
pub fn cell_code(rack_code: &str, grid: Grid, shelf: i32, cell: i32) -> String {
    let mut code = rack_code.to_owned();
    if grid.shelves > 1 {
        code.push_str(&format!("-{shelf}"));
    }
    if grid.cells_per_shelf > 1 {
        code.push_str(&format!("-{cell}"));
    }
    code
}

/// Меняются ли коды ячеек (а с ними наклейки): другой код шкафа или ось перешла через единицу.
pub fn codes_change(old_code: &str, old: Grid, new_code: &str, new: Grid) -> bool {
    old_code != new_code
        || (old.shelves > 1) != (new.shelves > 1)
        || (old.cells_per_shelf > 1) != (new.cells_per_shelf > 1)
}

/// Ячейки старой сетки, которых нет в новой.
pub fn lost_cells(old: Grid, new: Grid) -> Vec<(i32, i32)> {
    old.cells().filter(|&(s, c)| !new.contains(s, c)).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceTarget {
    Rack,
    Cell { shelf: i32, cell: i32 },
}

/// Часть кода до первого `-`: код шкафа.
pub fn rack_part(code: &str) -> &str {
    code.split('-').next().unwrap_or(code)
}

/// Что означает `code` (уже заглавными) в шкафу: сам шкаф или ячейку по каноническому коду.
pub fn place_in_rack(code: &str, rack_code: &str, grid: Grid) -> Option<PlaceTarget> {
    let single = grid.shelves == 1 && grid.cells_per_shelf == 1;
    if code == rack_code && !single {
        return Some(PlaceTarget::Rack);
    }
    grid.cells()
        .find(|&(shelf, cell)| cell_code(rack_code, grid, shelf, cell) == code)
        .map(|(shelf, cell)| PlaceTarget::Cell { shelf, cell })
}

/// Коды, которые мог иметь в виду человек: запрос как есть и с русской раскладки, заглавными.
pub fn place_candidates(query: &str) -> Vec<String> {
    let upper = query.trim().to_uppercase();
    let mut out: Vec<String> = Vec::new();
    for candidate in [upper.clone(), qwerty_from_jcuken(&upper).to_uppercase()] {
        if looks_like_place_code(&candidate) && !out.contains(&candidate) {
            out.push(candidate);
        }
    }
    out
}

/// `^[A-Z0-9]{1,8}(-[0-9]+){0,2}$`, в части шкафа есть буква.
fn looks_like_place_code(s: &str) -> bool {
    let mut parts = s.split('-');
    let rack = parts.next().unwrap_or_default();
    let mut numbers = 0;
    for part in parts {
        numbers += 1;
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
    }
    numbers <= 2 && normalize_rack_code(rack).is_some_and(|r| r == rack)
}

/// Порядок мест: код шкафа строкой, затем полка и ячейка.
pub fn compare_places(a: (&str, i32, i32), b: (&str, i32, i32)) -> Ordering {
    a.cmp(&b)
}

/// Сколько снять с каждого места, по порядку, чтобы сумма стала не больше `total`.
pub fn take_in_order(quantities: &[i64], total: i64) -> Vec<i64> {
    let mut excess = (quantities.iter().sum::<i64>() - total).max(0);
    quantities
        .iter()
        .map(|&q| {
            let take = q.min(excess);
            excess -= take;
            take
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(shelves: i32, cells_per_shelf: i32) -> Grid {
        Grid {
            shelves,
            cells_per_shelf,
        }
    }

    fn errs(grid: Grid) -> Vec<(String, String)> {
        grid.validate()
            .into_iter()
            .map(|e| (e.field, e.code))
            .collect()
    }

    fn pair(a: &str, b: &str) -> Vec<(String, String)> {
        vec![(a.to_owned(), b.to_owned())]
    }

    #[test]
    fn rack_codes_are_latin_letters_and_digits_with_a_letter() {
        assert_eq!(normalize_rack_code(" b2 "), Some("B2".to_owned()));
        assert!(normalize_rack_code("ABCDEFGH").is_some());
        assert!(normalize_rack_code("ABCDEFGHI").is_none());
        for bad in ["12", "", "Б1", "B-1", "B 1"] {
            assert!(normalize_rack_code(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn grids_are_1_to_20_by_1_to_20_and_at_most_100_cells() {
        assert_eq!(errs(g(0, 5)), pair("shelves", "out_of_range"));
        assert_eq!(errs(g(5, 21)), pair("cells_per_shelf", "out_of_range"));
        assert_eq!(errs(g(10, 11)), pair("cells_per_shelf", "rack_too_big"));
        assert!(errs(g(10, 10)).is_empty());
    }

    #[test]
    fn cell_codes_drop_the_axes_of_one() {
        assert_eq!(cell_code("B", g(3, 5), 2, 4), "B-2-4");
        assert_eq!(cell_code("B", g(3, 1), 2, 1), "B-2");
        assert_eq!(cell_code("B", g(1, 5), 1, 4), "B-4");
        assert_eq!(cell_code("B", g(1, 1), 1, 1), "B");
    }

    #[test]
    fn codes_change_only_when_an_axis_crosses_one_or_the_code_changes() {
        assert!(!codes_change("B", g(3, 5), "B", g(4, 6)));
        assert!(!codes_change("B", g(1, 1), "B", g(1, 1)));
        assert!(codes_change("B", g(3, 5), "B", g(3, 1)));
        assert!(codes_change("B", g(1, 5), "B", g(2, 5)));
        assert!(codes_change("B", g(3, 5), "C", g(3, 5)));
    }

    #[test]
    fn lost_cells_are_outside_the_new_grid() {
        assert_eq!(
            lost_cells(g(3, 3), g(2, 2)),
            vec![(1, 3), (2, 3), (3, 1), (3, 2), (3, 3)]
        );
        assert!(lost_cells(g(2, 2), g(3, 3)).is_empty());
    }

    #[test]
    fn places_are_found_only_by_their_canonical_code() {
        let big = g(3, 5);
        assert_eq!(
            place_in_rack("B-2-4", "B", big),
            Some(PlaceTarget::Cell { shelf: 2, cell: 4 })
        );
        assert_eq!(place_in_rack("B", "B", big), Some(PlaceTarget::Rack));
        for bad in ["B-02-4", "B-4-1", "B-2", "B-2-4-1"] {
            assert_eq!(place_in_rack(bad, "B", big), None, "{bad}");
        }
        assert_eq!(
            place_in_rack("B-2", "B", g(3, 1)),
            Some(PlaceTarget::Cell { shelf: 2, cell: 1 })
        );
        assert_eq!(
            place_in_rack("B", "B", g(1, 1)),
            Some(PlaceTarget::Cell { shelf: 1, cell: 1 })
        );
    }

    #[test]
    fn candidates_read_the_russian_layout() {
        assert_eq!(place_candidates("и-2-4"), vec!["B-2-4"]);
        assert_eq!(place_candidates("И-2-4"), vec!["B-2-4"]);
        assert_eq!(place_candidates(" b-2-4 "), vec!["B-2-4"]);
        assert_eq!(place_candidates("ыфь"), vec!["SAM"]);
        for none in ["41", "дисплей самсунг", "B 2"] {
            assert!(place_candidates(none).is_empty(), "{none}");
        }
    }

    #[test]
    fn take_in_order_takes_until_the_sum_fits() {
        assert_eq!(take_in_order(&[2, 3], 2), vec![2, 1]);
        assert_eq!(take_in_order(&[2, 3], 5), vec![0, 0]);
        assert_eq!(take_in_order(&[2, 3], 0), vec![2, 3]);
        assert_eq!(take_in_order(&[], 0), Vec::<i64>::new());
    }

    #[test]
    fn places_sort_by_rack_code_then_numbers() {
        use std::cmp::Ordering::Less;
        assert_eq!(compare_places(("B", 1, 2), ("B", 1, 10)), Less);
        assert_eq!(compare_places(("A10", 1, 1), ("A9", 1, 1)), Less);
        assert_eq!(compare_places(("A", 2, 1), ("B", 1, 1)), Less);
    }
}
