//! Флаги старой базы: «1», «True», «Да», «Заказать» и подобные.

/// `Some(true/false)` для известных значений, иначе `None`.
pub(crate) fn parse_flag(raw: &str) -> Option<bool> {
    match raw.trim().to_lowercase().as_str() {
        "1" | "true" | "да" | "yes" | "checked" | "заказать" => Some(true),
        "" | "0" | "-1" | "false" | "нет" | "no" | "unchecked" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags() {
        for t in [
            "1",
            "True",
            "Да",
            "Заказать",
            " заказать ",
            "Checked",
            "YES",
        ] {
            assert_eq!(parse_flag(t), Some(true), "{t}");
        }
        for f in ["", "0", "-1", "False", "нет", "Unchecked", " ", "\u{a0}"] {
            assert_eq!(parse_flag(f), Some(false), "{f}");
        }
        assert_eq!(parse_flag("что-то"), None);
        assert_eq!(parse_flag("2"), None);
    }
}
