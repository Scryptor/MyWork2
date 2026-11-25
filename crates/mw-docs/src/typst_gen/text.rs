//! Template text as Typst code: literal text, line breaks and `{placeholders}` replaced by values.

use std::fmt::Write;

use crate::fields::{FieldDef, field_by_placeholder, link_by_placeholder};

pub(super) enum Piece {
    Text(String),
    Field(&'static FieldDef),
    /// A firm link by its key `link.<id>`: a value like a field's, empty when the link is off or missing.
    Link(String),
    Break,
}

impl Piece {
    /// The key of the value a substitution takes; `None` for text and breaks.
    pub(super) fn key(&self) -> Option<&str> {
        match self {
            Piece::Field(field) => Some(field.key),
            Piece::Link(key) => Some(key),
            Piece::Text(_) | Piece::Break => None,
        }
    }
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
                let found = rest[1..]
                    .find(['{', '}'])
                    .filter(|&end| rest[1 + end..].starts_with('}'))
                    .and_then(|end| {
                        let name = &rest[1..=end];
                        let piece = field_by_placeholder(name)
                            .map(Piece::Field)
                            .or_else(|| link_by_placeholder(name).map(Piece::Link));
                        piece.map(|piece| (piece, end + 2))
                    });
                match found {
                    Some((piece, consumed)) => {
                        flush(&mut pieces, &mut literal);
                        pieces.push(piece);
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
            (Piece::Break, TextMode::OneLine) => "\" \"".to_owned(),
            (Piece::Break, _) => "linebreak()".to_owned(),
            (Piece::Field(_) | Piece::Link(_), mode) => {
                let key = typst_str(p.key().unwrap_or_default());
                match mode {
                    TextMode::Wrap => format!("v({key})"),
                    TextMode::ShortLine => format!("v({key}, line: short-line)"),
                    TextMode::OneLine => format!("v1({key})"),
                }
            }
        })
        .collect();
    join_parts(parts)
}

/// A Typst code expression for a list: `text` with its substitutions taken as raw values, every
/// non-empty line (a typed number «1.», «2)» at its start dropped) is an item of a numbered list.
pub(super) fn numbered_expr(text: &str) -> String {
    let parts: Vec<String> = pieces(text, true)
        .iter()
        .map(|p| match p {
            Piece::Text(text) => typst_str(text),
            Piece::Break => "\"\\n\"".to_owned(),
            Piece::Field(_) | Piece::Link(_) => {
                format!("raw-value({})", typst_str(p.key().unwrap_or_default()))
            }
        })
        .collect();
    let source = if parts.is_empty() {
        "\"\"".to_owned()
    } else {
        parts.join(" + ")
    };
    format!(
        r#"{{ let items = ({source}).replace("\r\n", "\n").replace("\r", "\n").split("\n").map(l => l.trim().replace(regex("^\\d{{1,3}}[.)]\\s*"), "").trim()).filter(l => l != ""); if items.len() > 0 {{ enum(..items) }} }}"#
    )
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
        assert_eq!(
            text_expr(
                "{ссылка.telegram} {Фирма.Телеграм} {ссылка.Bad-Id}",
                TextMode::OneLine
            ),
            r#"(v1("link.telegram"), " ", v1("link.telegram"), " {ссылка.Bad-Id}").join()"#
        );
        assert_eq!(text_expr("", TextMode::Wrap), r#""""#);
    }
}
