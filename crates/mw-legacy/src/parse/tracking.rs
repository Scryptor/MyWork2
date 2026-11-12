//! Почтовые трек-номера в комментариях: `###НОМЕР описание` (как `###(\w+)(.*)` в MyWork 2).

/// Пары (номер, описание) по одной на строку с `###`.
pub(crate) fn extract_tracking(comment: &str) -> Vec<(String, String)> {
    comment.lines().filter_map(track_in_line).collect()
}

/// Первая `###`, сразу за которой идёт буква, цифра или `_` (любой алфавит).
fn track_in_line(line: &str) -> Option<(String, String)> {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let start = (0..line.len())
        .filter(|&i| line.is_char_boundary(i) && line[i..].starts_with("###"))
        .find(|&i| line[i + 3..].chars().next().is_some_and(is_word))?;
    let rest = &line[start + 3..];
    let end = rest.find(|c: char| !is_word(c)).unwrap_or(rest.len());
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
    fn first_marker_followed_by_a_word_char_wins() {
        assert_eq!(
            extract_tracking("текст ###AB12 раз ###CD34"),
            vec![("AB12".to_owned(), "раз ###CD34".to_owned())]
        );
        assert_eq!(
            extract_tracking("### x ###AB12 текст"),
            vec![("AB12".to_owned(), "текст".to_owned())]
        );
        assert_eq!(
            extract_tracking("####AB12"),
            vec![("AB12".to_owned(), String::new())]
        );
    }
}
