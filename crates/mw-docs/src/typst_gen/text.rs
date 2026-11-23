//! Template text as Typst code: literal text, line breaks and `{placeholders}` replaced by values.

use std::fmt::Write;

use crate::fields::{FieldDef, field_by_placeholder};

pub(super) enum Piece {
    Text(String),
    Field(&'static FieldDef),
    Break,
}

/// Splits template text into literal text, line breaks and (with `substitutions`) known `{placeholders}`.
pub(super) fn pieces(text: &str, substitutions: bool) -> Vec<Piece> {
    let mut pieces = Vec::new();
    let mut literal = String::new();
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        let mut len = c.len_utf8();
        match c {
            '{' if substitutions => {
                // A name runs to the next brace; a `{` there means this one is literal text.
                let field = rest[1..]
                    .find(['{', '}'])
                    .filter(|&end| rest[1 + end..].starts_with('}'))
                    .and_then(|end| field_by_placeholder(&rest[1..=end]).map(|f| (f, end + 2)));
                match field {
                    Some((field, consumed)) => {
                        flush(&mut pieces, &mut literal);
                        pieces.push(Piece::Field(field));
                        len = consumed;
                    }
                    None => literal.push(c),
                }
            }
            '\r' | '\n' => {
                flush(&mut pieces, &mut literal);
                pieces.push(Piece::Break);
                if rest.starts_with("\r\n") {
                    len = 2;
                }
            }
            _ => literal.push(c),
        }
        rest = &rest[len..];
    }
    flush(&mut pieces, &mut literal);
    pieces
}

fn flush(pieces: &mut Vec<Piece>, literal: &mut String) {
    if !literal.is_empty() {
        pieces.push(Piece::Text(std::mem::take(literal)));
    }
}

/// How [`text_expr`] writes line breaks and empty values.
#[derive(Clone, Copy)]
pub(super) enum TextMode {
    /// Values through `v`, line breaks as `linebreak()`.
    Wrap,
    /// [`TextMode::Wrap`] with short lines to write on (centred text).
    ShortLine,
    /// One line: values through `v1`, line breaks as spaces.
    OneLine,
}

/// One Typst code expression for `text` with its `{placeholders}` substituted.
pub(super) fn text_expr(text: &str, mode: TextMode) -> String {
    pieces_expr(&pieces(text, true), mode)
}

/// One Typst code expression for `text` taken literally: no substitutions, only line breaks.
pub(super) fn literal_expr(text: &str) -> String {
    pieces_expr(&pieces(text, false), TextMode::Wrap)
}

fn pieces_expr(pieces: &[Piece], mode: TextMode) -> String {
    let parts: Vec<String> = pieces
        .iter()
        .map(|p| match (p, mode) {
            (Piece::Text(text), _) => typst_str(text),
            (Piece::Field(field), TextMode::Wrap) => format!("v({})", typst_str(field.key)),
            (Piece::Field(field), TextMode::ShortLine) => {
                format!("v({}, line: short-line)", typst_str(field.key))
            }
            (Piece::Field(field), TextMode::OneLine) => format!("v1({})", typst_str(field.key)),
            (Piece::Break, TextMode::OneLine) => "\" \"".to_owned(),
            (Piece::Break, _) => "linebreak()".to_owned(),
        })
        .collect();
    join_parts(parts)
}

fn join_parts(parts: Vec<String>) -> String {
    match parts.as_slice() {
        [] => "\"\"".to_owned(),
        [one] => one.clone(),
        _ => format!("({}).join()", parts.join(", ")),
    }
}

/// A Typst string literal: the text exactly, whatever characters it has.
pub(super) fn typst_str(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{{{:x}}}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_literals_escape_quotes_backslashes_and_controls() {
        assert_eq!(typst_str(r#"a\b"c"#), r#""a\\b\"c""#);
        assert_eq!(typst_str("x\ny\t\u{7}"), r#""x\ny\t\u{7}""#);
        assert_eq!(
            typst_str("#set page() $x$ // к"),
            "\"#set page() $x$ // к\""
        );
    }

    #[test]
    fn pieces_find_known_placeholders_only() {
        assert_eq!(
            text_expr("№ {заказ.номер}{нет}\r\n{{клиент.фио}", TextMode::Wrap),
            r#"("№ ", v("order.number"), "{нет}", linebreak(), "{", v("client.name")).join()"#
        );
        assert_eq!(literal_expr("{заказ.номер}"), r#""{заказ.номер}""#);
        assert_eq!(text_expr("", TextMode::Wrap), r#""""#);
    }
}
