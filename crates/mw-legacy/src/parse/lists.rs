//! Поля-списки заказа MyWork 2 → вид MyWork 3 «пункт, пункт».
//!
//! MyWork 2 дописывала выбранный в выпадающем списке пункт в текст поля: неисправность — «пункт/ »
//! (`AddPosition.cs`; сама она делила это поле по «/»), выполненные работы — пункт и перевод строки
//! (`Editor.cs`), состояние — «пункт, », комплектность — пункт без разделителя («Аппарат» + «АКБ» =
//! «АппаратАКБ»). Кроме того, в эти поля писали руками — через запятую и с новой строки.

use mw_domain::spelling::tidy;

/// Поле-список заказа MyWork 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ListField {
    /// `komplektonst`.
    Completeness,
    /// `sostoyanie`.
    Condition,
    /// `polomka`.
    Malfunction,
    /// `vipolnenie_raboti`.
    WorkDone,
}

/// Знаки, которые обрезаются в конце пункта.
const TRAILING: [char; 4] = ['.', ';', '/', ','];

/// Пункт без краёв, повторных пробелов и конечных «.», «;», «/», «,»: «Не включается.» —
/// «Не включается».
pub(crate) fn clean_item(text: &str) -> String {
    let mut item = tidy(text);
    while let Some(c) = item.chars().last().filter(|c| TRAILING.contains(c)) {
        item.truncate(item.len() - c.len_utf8());
        item.truncate(item.trim_end().len());
    }
    item
}

/// Разделитель пунктов в поле: запятая и перевод строки везде, «/» — у неисправности.
fn is_separator(field: ListField, c: char) -> bool {
    c == ',' || c == '\n' || c == '\r' || (field == ListField::Malfunction && c == '/')
}

/// Пункты поля по порядку (пустые пропущены, повторы остаются). Комплектность, склеенная из строк
/// выпадающего списка MyWork 2 (`offered`), делится по ним — только если кусок целиком состоит хотя
/// бы из двух таких строк (без учёта регистра, пробелы между ними допускаются).
pub(crate) fn split_list(field: ListField, text: &str, offered: &[String]) -> Vec<String> {
    let mut items = Vec::new();
    for piece in text.split(|c| is_separator(field, c)) {
        let piece = clean_item(piece);
        if piece.is_empty() {
            continue;
        }
        match (field == ListField::Completeness)
            .then(|| unglue(&piece, offered))
            .flatten()
        {
            Some(parts) => items.extend(parts),
            None => items.push(piece),
        }
    }
    items
}

/// Текст поля в виде MyWork 3: пункты через «, ».
pub(crate) fn list_text(field: ListField, text: &str, offered: &[String]) -> String {
    split_list(field, text, offered).join(", ")
}

/// Кусок, склеенный из строк списка: сами строки (их написание) или `None`, если кусок — одна
/// строка списка или не складывается из них целиком.
fn unglue(piece: &str, offered: &[String]) -> Option<Vec<String>> {
    let lower = piece.to_lowercase();
    let mut words: Vec<(String, &str)> = offered
        .iter()
        .map(|o| o.trim())
        .filter(|o| !o.is_empty())
        .map(|o| (o.to_lowercase(), o))
        .collect();
    if words.iter().any(|(w, _)| *w == lower) {
        return None;
    }
    // Сначала длинные: «Зарядное устройство» раньше «Зарядное».
    words.sort_by_key(|(w, _)| std::cmp::Reverse(w.len()));
    // Откуда пришли в позицию (байт строки `lower`): предыдущая позиция и строка списка.
    let mut came: Vec<Option<(usize, usize)>> = vec![None; lower.len() + 1];
    let mut reached = vec![false; lower.len() + 1];
    reached[0] = true;
    for pos in 0..lower.len() {
        if !reached[pos] || !lower.is_char_boundary(pos) {
            continue;
        }
        let rest = &lower[pos..];
        let start = pos + (rest.len() - rest.trim_start().len());
        for (i, (word, _)) in words.iter().enumerate() {
            let end = start + word.len();
            if lower[start..].starts_with(word.as_str()) && !reached[end] {
                reached[end] = true;
                came[end] = Some((pos, i));
            }
        }
    }
    let mut pos = lower.trim_end().len();
    let mut parts = Vec::new();
    while pos > 0 {
        let (prev, i) = came[pos]?;
        parts.push(words[i].1.to_owned());
        pos = prev;
    }
    parts.reverse();
    (parts.len() >= 2).then_some(parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offered() -> Vec<String> {
        [
            "Аппарат",
            "АКБ",
            "Зарядное устройство",
            "Чехол",
            "Блок питания",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect()
    }

    fn items(field: ListField, text: &str) -> Vec<String> {
        split_list(field, text, &offered())
    }

    #[test]
    fn malfunction_is_split_by_slash_as_mywork2_writes_it() {
        assert_eq!(
            items(ListField::Malfunction, "Не включается/ Разбит экран/ "),
            ["Не включается", "Разбит экран"]
        );
        assert_eq!(
            items(
                ListField::Malfunction,
                "Не включается/Залит, не заряжается\r\nшумит"
            ),
            ["Не включается", "Залит", "не заряжается", "шумит"]
        );
        assert_eq!(
            list_text(ListField::Malfunction, "Течёт/ течёт/ ", &[]),
            "Течёт, течёт"
        );
        assert!(items(ListField::Malfunction, " / / ").is_empty());
    }

    #[test]
    fn work_done_is_split_by_lines_and_condition_by_commas() {
        assert_eq!(
            items(ListField::WorkDone, "Замена дисплея\r\nЧистка\r\n"),
            ["Замена дисплея", "Чистка"]
        );
        // У работ «/» — часть текста.
        assert_eq!(
            items(ListField::WorkDone, "Замена разъёма micro/USB"),
            ["Замена разъёма micro/USB"]
        );
        assert_eq!(
            items(ListField::Condition, "Царапины, Потёртости, "),
            ["Царапины", "Потёртости"]
        );
    }

    #[test]
    fn trailing_punctuation_and_spaces_are_trimmed() {
        assert_eq!(clean_item(" Не  включается. "), "Не включается");
        assert_eq!(clean_item("Не включается;/ ,"), "Не включается");
        assert_eq!(clean_item("т.д."), "т.д");
        assert_eq!(clean_item("..."), "");
        assert_eq!(clean_item("Wi-Fi"), "Wi-Fi");
        assert_eq!(
            items(ListField::Condition, "Царапины.\nсколы;"),
            ["Царапины", "сколы"]
        );
    }

    #[test]
    fn completeness_glued_from_the_list_is_split_by_its_lines() {
        assert_eq!(
            items(ListField::Completeness, "АппаратАКБ"),
            ["Аппарат", "АКБ"]
        );
        assert_eq!(
            items(ListField::Completeness, "аппаратЗарядное устройствоЧехол"),
            ["Аппарат", "Зарядное устройство", "Чехол"]
        );
        // Пробел после строки списка (набрано руками) — тоже склейка.
        assert_eq!(
            items(ListField::Completeness, "Аппарат Блок питания, коробка"),
            ["Аппарат", "Блок питания", "коробка"]
        );
        // Одна строка списка и текст, который из них не складывается, — как есть.
        assert_eq!(items(ListField::Completeness, "Аппарат"), ["Аппарат"]);
        assert_eq!(
            items(ListField::Completeness, "Аппарат без АКБ"),
            ["Аппарат без АКБ"]
        );
        assert_eq!(
            items(ListField::Completeness, "АппаратАКБ2"),
            ["АппаратАКБ2"]
        );
        // Без списка — не делится.
        assert_eq!(
            split_list(ListField::Completeness, "АппаратАКБ", &[]),
            ["АппаратАКБ"]
        );
        // Другие поля не склеиваются.
        assert_eq!(items(ListField::Condition, "АппаратАКБ"), ["АппаратАКБ"]);
    }
}
