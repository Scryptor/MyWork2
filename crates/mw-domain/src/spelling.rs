//! Написание значения, собранного из многих записей: имена мастеров (перенос из MyWork 2, миграция
//! «мастера → пользователи», начальная настройка) и значения справочников при переносе.
//!
//! Записи одного значения сводятся без учёта регистра (`crate::search::normalize_search`); здесь —
//! какое из написаний взять и какие тексты значением не считать.

/// Есть ли в тексте хоть одна буква. Текст без букв («-», «--», «?», «0», «№ 5») — не имя и не
/// значение справочника.
pub fn has_letter(text: &str) -> bool {
    text.chars().any(char::is_alphabetic)
}

/// Края обрезаны, повторные пробельные символы внутри — один пробел.
pub fn tidy(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Все буквы заглавные (и хоть одна буква с регистром есть): «ИВАНОВ», «HP».
pub fn is_all_caps(text: &str) -> bool {
    let mut cased = false;
    for c in text.chars() {
        if c.is_lowercase() {
            return false;
        }
        cased |= c.is_uppercase();
    }
    cased
}

/// Написания одного значения с числом записей, в порядке первого появления.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Spellings(Vec<(String, u64)>);

impl Spellings {
    /// Ещё `count` записей с написанием `spelling` (как есть: приводит вызывающий, обычно `tidy`).
    pub fn add(&mut self, spelling: &str, count: u64) {
        match self.0.iter_mut().find(|(s, _)| s == spelling) {
            Some((_, n)) => *n += count,
            None => self.0.push((spelling.to_owned(), count)),
        }
    }

    /// Имя человека (мастер): самое частое написание; при равенстве — не заглавными буквами, затем
    /// первое по появлению.
    pub fn best_name(&self) -> Option<&str> {
        self.0
            .iter()
            .enumerate()
            .min_by_key(|(i, (spelling, count))| {
                (std::cmp::Reverse(*count), is_all_caps(spelling), *i)
            })
            .map(|(_, (spelling, _))| spelling.as_str())
    }

    /// Значение справочника: самое частое написание, при равенстве — первое по появлению
    /// (заглавные не уступают: «LG», «ZTE»).
    pub fn most_frequent(&self) -> Option<&str> {
        self.most_frequent_among(|_| true)
    }

    /// Самое частое из написаний, подходящих под `prefer`; если не подходит ни одно — из всех.
    pub fn most_frequent_among(&self, prefer: impl Fn(&str) -> bool) -> Option<&str> {
        let any = self.0.iter().any(|(s, _)| prefer(s));
        self.0
            .iter()
            .enumerate()
            .filter(|(_, (s, _))| !any || prefer(s))
            .min_by_key(|(i, (_, count))| (std::cmp::Reverse(*count), *i))
            .map(|(_, (spelling, _))| spelling.as_str())
    }

    /// Сколько записей всего.
    pub fn total(&self) -> u64 {
        self.0.iter().map(|(_, n)| n).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_without_a_letter_is_junk() {
        for junk in ["", " ", "-", "--", "?", "0", "№ 5", "+7 (921) 000", "…"] {
            assert!(!has_letter(junk), "{junk}");
        }
        for name in ["Иванов", "a", "3M", "ё", "Мастер 1", "Ǆ"] {
            assert!(has_letter(name), "{name}");
        }
    }

    #[test]
    fn tidy_trims_and_collapses_inner_spaces() {
        assert_eq!(tidy("  Иванов \t  Иван\u{a0} "), "Иванов Иван");
        assert_eq!(tidy("Иванов"), "Иванов");
        assert_eq!(tidy("   "), "");
    }

    #[test]
    fn all_caps_needs_a_cased_letter_and_no_lowercase() {
        assert!(is_all_caps("ИВАНОВ И.И."));
        assert!(is_all_caps("HP 15"));
        assert!(!is_all_caps("Иванов"));
        assert!(!is_all_caps("iPhone"));
        assert!(!is_all_caps("123"));
        assert!(!is_all_caps(""));
    }

    #[test]
    fn a_name_takes_the_most_frequent_spelling_then_not_all_caps_then_the_first() {
        let best = |list: &[(&str, u64)]| {
            let mut s = Spellings::default();
            for (spelling, n) in list {
                s.add(spelling, *n);
            }
            s.best_name().map(str::to_owned)
        };
        assert_eq!(best(&[]), None);
        assert_eq!(
            best(&[("nokia", 1), ("Nokia", 2)]).as_deref(),
            Some("Nokia")
        );
        assert_eq!(
            best(&[("ИВАНОВ", 2), ("Иванов", 1)]).as_deref(),
            Some("ИВАНОВ")
        );
        assert_eq!(
            best(&[("ИВАНОВ", 1), ("Иванов", 1)]).as_deref(),
            Some("Иванов")
        );
        assert_eq!(
            best(&[("петров", 1), ("Петров", 1)]).as_deref(),
            Some("петров")
        );
        assert_eq!(best(&[("HP", 1), ("HTC", 1)]).as_deref(), Some("HP"));
        // Повторное написание складывается с первым.
        assert_eq!(
            best(&[("Иванов", 1), ("ИВАНОВ", 1), ("ИВАНОВ", 1)]).as_deref(),
            Some("ИВАНОВ")
        );
        let mut s = Spellings::default();
        s.add("a", 2);
        s.add("A", 3);
        assert_eq!(s.total(), 5);
    }

    #[test]
    fn a_value_takes_the_most_frequent_spelling_then_the_first_even_in_capitals() {
        let spellings = |list: &[(&str, u64)]| {
            let mut s = Spellings::default();
            for (spelling, n) in list {
                s.add(spelling, *n);
            }
            s
        };
        let most = |list: &[(&str, u64)]| spellings(list).most_frequent().map(str::to_owned);
        assert_eq!(most(&[]), None);
        assert_eq!(most(&[("LG", 1), ("Lg", 1)]).as_deref(), Some("LG"));
        assert_eq!(most(&[("Lg", 1), ("LG", 2)]).as_deref(), Some("LG"));
        assert_eq!(most(&[("zte", 1), ("ZTE", 1)]).as_deref(), Some("zte"));
        // Предпочтение: с буквами — даже реже; без подходящих — из всех.
        let with_unit = |list: &[(&str, u64)]| {
            spellings(list)
                .most_frequent_among(has_letter)
                .map(str::to_owned)
        };
        assert_eq!(
            with_unit(&[("30", 5), ("30 дней", 1), ("30 дн", 1)]).as_deref(),
            Some("30 дней")
        );
        assert_eq!(with_unit(&[("30", 1), ("90", 2)]).as_deref(), Some("90"));
    }
}
