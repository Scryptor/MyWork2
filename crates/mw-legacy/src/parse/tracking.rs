//! Почтовые трек-номера в комментариях: `###НОМЕР описание` (как `###(\w+)(.*)` в MyWork 2).

/// Пары (номер, описание) по одной на строку с `###`.
pub(crate) fn extract_tracking(comment: &str) -> Vec<(String, String)> {
    comment.lines().filter_map(track_in_line).collect()
}

fn track_in_line(line: &str) -> Option<(String, String)> {
    let rest = &line[line.find("###")? + 3..];
    let end = rest
        .find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(rest.len());
    if end == 0 {
        return None;
    }
    Some((rest[..end].to_owned(), rest[end..].trim().to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_numbers_and_descriptions() {
        assert_eq!(
            extract_tracking("Позвонить\r\n###RA123456789RU посылка\r\n###80081234567890"),
            vec![
                ("RA123456789RU".to_owned(), "посылка".to_owned()),
                ("80081234567890".to_owned(), String::new())
            ]
        );
        assert!(extract_tracking("### пусто").is_empty());
        assert!(extract_tracking("без треков").is_empty());
        assert!(extract_tracking("").is_empty());
    }

    #[test]
    fn only_first_marker_in_a_line_counts() {
        assert_eq!(
            extract_tracking("текст ###AB12 раз ###CD34"),
            vec![("AB12".to_owned(), "раз ###CD34".to_owned())]
        );
    }
}
