//! The QR blocks of a document: the name of each code's file and the files themselves. The link
//! and its values go only into the files; the markup knows a code by its name.

use std::collections::{BTreeMap, BTreeSet};

use crate::fields::{LEGACY_LINKS, LINK_KEY_PREFIX};
use crate::model::{Block, BlockBody, Template};
use crate::qr::qr_svg;

use super::text::{Piece, pieces};

/// The name of the code of a QR block with `link` at `path` (`[i]`, or `[i, column, j]` inside
/// columns), the document's file being `qr-<name>.svg`. A link of exactly one firm-link substitution,
/// spaces around it aside, is named by the link's key (`link.telegram`): every such block shows one
/// file. Any other link is the block's own, `block-<path joined by «-»>`.
pub(super) fn qr_name(link: &str, path: &[usize]) -> String {
    let mut links = Vec::new();
    let mut other = false;
    for piece in pieces(link, true) {
        match piece {
            Piece::Link(key) => links.push(key),
            Piece::Field(_) => other = true,
            Piece::Text(t) => other |= !t.trim().is_empty(),
            Piece::Break => {}
        }
    }
    match links.as_slice() {
        [key] if !other => key.clone(),
        _ => {
            let at: Vec<String> = path.iter().map(usize::to_string).collect();
            format!("block-{}", at.join("-"))
        }
    }
}

/// `link` with the `values` substituted and the spaces at its ends trimmed. `None` when a substitution
/// in it is empty (half a link is worse than none) or nothing is left.
pub(super) fn link_value(link: &str, values: &BTreeMap<String, String>) -> Option<String> {
    let mut out = String::new();
    for piece in pieces(link, true) {
        match &piece {
            Piece::Text(text) => out.push_str(text),
            Piece::Field(_) | Piece::Link(_) => {
                let value = values
                    .get(piece.key().unwrap_or_default())
                    .map_or("", String::as_str);
                if value.trim().is_empty() {
                    return None;
                }
                out.push_str(value);
            }
            Piece::Break => out.push('\n'),
        }
    }
    let out = out.trim();
    (!out.is_empty()).then(|| out.to_owned())
}

/// The codes of the QR blocks of `template`, columns included, in the order of the blocks: (name,
/// SVG), one per name. A block whose link is empty or does not fit a QR code has none. A template
/// with code also gets the non-empty links of the firm that no block made.
pub(super) fn qr_files(
    template: &Template,
    values: &BTreeMap<String, String>,
) -> Vec<(String, String)> {
    let mut links = Vec::new();
    qr_links(&template.blocks, &[], &mut links);
    let mut names = BTreeSet::new();
    let mut files = Vec::new();
    for (path, link) in links {
        let name = qr_name(link, &path);
        if !names.insert(name.clone()) {
            continue;
        }
        if let Some(svg) = link_value(link, values).and_then(|value| qr_svg(&value)) {
            files.push((name, svg));
        }
    }
    // The author's code reads the firm's links by their keys, whatever blocks there are. Code made
    // before the list of links asks `data.qr` for the old names `firm.*` too: the three old links
    // get their file under both names.
    if template.has_code() {
        for (key, value) in values {
            let value = value.trim();
            if !key.starts_with(LINK_KEY_PREFIX) || value.is_empty() || names.contains(key) {
                continue;
            }
            if let Some(svg) = qr_svg(value) {
                files.push((key.clone(), svg));
            }
        }
        for legacy in LEGACY_LINKS {
            let link_key = format!("{LINK_KEY_PREFIX}{}", legacy.id);
            let svg = files
                .iter()
                .find(|(name, _)| *name == link_key)
                .map(|(_, svg)| svg.clone());
            if let Some(svg) = svg.filter(|_| !names.contains(legacy.key)) {
                files.push((legacy.key.to_owned(), svg));
            }
        }
    }
    files
}

/// Appends (path, link) of every QR block in `blocks`, which are at `path`.
fn qr_links<'a>(blocks: &'a [Block], path: &[usize], out: &mut Vec<(Vec<usize>, &'a str)>) {
    for (i, block) in blocks.iter().enumerate() {
        let at = [path, &[i]].concat();
        match &block.body {
            BlockBody::Qr { link, .. } => out.push((at, link)),
            BlockBody::Columns { columns, .. } => {
                for (c, column) in columns.iter().enumerate() {
                    qr_links(column, &[at.as_slice(), &[c]].concat(), out);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Align, CopyFilter};
    use crate::standard::standard_template;
    use mw_domain::documents::DocumentKind;

    fn values(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    fn qr(link: &str) -> Block {
        Block {
            copies: CopyFilter::Both,
            body: BlockBody::Qr {
                link: link.to_owned(),
                size_mm: 20.0,
                caption: String::new(),
                align: Align::Center,
            },
        }
    }

    #[test]
    fn a_firm_link_alone_is_named_by_its_key() {
        assert_eq!(qr_name("{фирма.телеграм}", &[3]), "link.telegram");
        assert_eq!(qr_name("{ссылка.telegram}", &[3]), "link.telegram");
        assert_eq!(qr_name("  {Фирма.Гугл_Карты}\n", &[3]), "link.google_maps");
        assert_eq!(
            qr_name("{фирма.яндекс_карты}", &[2, 0, 1]),
            "link.yandex_maps"
        );
        assert_eq!(qr_name("{ссылка.link_gone}", &[0]), "link.link_gone");
        // Anything else is named by the block.
        assert_eq!(qr_name("{фирма.телеграм}?x=1", &[3]), "block-3");
        assert_eq!(qr_name("{фирма.телеграм}{фирма.телеграм}", &[3]), "block-3");
        assert_eq!(qr_name("{фирма.телефон}", &[2, 0, 1]), "block-2-0-1");
        assert_eq!(qr_name("https://example.com", &[0]), "block-0");
        assert_eq!(qr_name("", &[0]), "block-0");
    }

    #[test]
    fn link_value_substitutes_and_trims() {
        let v = values(&[
            ("order.number", "1234"),
            ("link.telegram", " https://t.me/x "),
            ("order.color", "  "),
        ]);
        let value = |link| link_value(link, &v);
        assert_eq!(
            value(" https://example.com/o/{заказ.номер} ").as_deref(),
            Some("https://example.com/o/1234")
        );
        assert_eq!(value("{фирма.телеграм}").as_deref(), Some("https://t.me/x"));
        assert_eq!(value("a\nb").as_deref(), Some("a\nb"));
        assert_eq!(value("{нет}").as_deref(), Some("{нет}"));
        // An empty substitution, or nothing at all, is no link.
        assert_eq!(value("https://example.com/{заказ.цвет}"), None);
        assert_eq!(value("https://example.com/{клиент.фио}"), None);
        assert_eq!(value(" \n "), None);
        assert_eq!(value(""), None);
    }

    #[test]
    fn one_file_per_name_and_none_without_a_link() {
        let mut t = standard_template(DocumentKind::IntakeAct);
        t.blocks = vec![
            qr("{фирма.телеграм}"),
            qr("https://example.com/{заказ.номер}"),
            qr("{фирма.яндекс_карты}"),
            Block {
                copies: CopyFilter::Both,
                body: BlockBody::Columns {
                    columns: vec![vec![qr(" {фирма.телеграм} ")], vec![qr("свой")]],
                    gap_mm: 2.0,
                },
            },
        ];
        let v = values(&[("link.telegram", "https://t.me/x"), ("order.number", "")]);
        let files = qr_files(&t, &v);
        let names: Vec<&str> = files.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(names, ["link.telegram", "block-3-1-0"]);
        assert_eq!(files[0].1, qr_svg("https://t.me/x").expect("fits"));
        assert_eq!(files[1].1, qr_svg("свой").expect("fits"));
        // A link that does not fit a QR code gets no file.
        t.blocks = vec![qr("{фирма.сведения}")];
        let long = "a".repeat(3000);
        assert!(qr_files(&t, &values(&[("firm.details", &long)])).is_empty());
    }
}
