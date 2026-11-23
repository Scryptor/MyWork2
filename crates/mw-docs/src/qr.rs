//! QR codes as SVG: the dark modules only. The room around the code is left by the document.

use std::fmt::Write;

use qrcode::bits::Bits;
use qrcode::{Color, EcLevel, QrCode, Version};

/// The most bytes a symbol holds as one byte-mode segment at level M: the capacity of version 40.
/// A longer text fits no version, and is not tried on all 40.
const MAX_BYTES: usize = 2331;

/// SVG of `text` as a QR code of error correction level M: one unit per module (`viewBox="0 0 n n"`,
/// `n` the side of the symbol in modules), no quiet zone, one `<path>` of the runs of dark modules
/// row by row. `None` when the text does not fit a QR code.
///
/// The UTF-8 bytes of the text are one byte-mode segment, without an ECI designator, in the smallest
/// version that holds them. Not the automatic choice of modes: it takes byte pairs that look like
/// Shift-JIS for Kanji, and a Cyrillic letter followed by another often does (the second byte of
/// А–П and с–я is 0x81–0x9F, the next letter starts with 0xD0 or 0xD1); phones read such a segment
/// as Japanese characters.
pub(crate) fn qr_svg(text: &str) -> Option<String> {
    if text.len() > MAX_BYTES {
        return None;
    }
    let code = (1..=40).find_map(|version| byte_mode(text.as_bytes(), version))?;
    let n = code.width();
    let mut path = String::new();
    for (y, row) in code.to_colors().chunks(n).enumerate() {
        let mut x = 0;
        while x < n {
            let run = row[x..].iter().take_while(|c| **c == Color::Dark).count();
            if run > 0 {
                let _ = write!(path, "M{x} {y}h{run}v1h-{run}z");
            }
            x += run.max(1);
        }
    }
    Some(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {n} {n}\" \
         shape-rendering=\"crispEdges\"><path d=\"{path}\"/></svg>"
    ))
}

/// `data` as one byte-mode segment in a symbol of `version`; `None` when it does not fit.
fn byte_mode(data: &[u8], version: i16) -> Option<QrCode> {
    let mut bits = Bits::new(Version::Normal(version));
    bits.push_byte_data(data).ok()?;
    bits.push_terminator(EcLevel::M).ok()?;
    QrCode::with_bits(bits, EcLevel::M).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TELEGRAM: &str = "https://t.me/example_service";

    /// The side of `svg` in modules and its dark modules, row by row, read back from its path.
    fn modules(svg: &str) -> (usize, Vec<bool>) {
        let view = svg.split("viewBox=\"0 0 ").nth(1).expect("viewBox");
        let (n, _) = view.split_once(' ').expect("side");
        let n: usize = n.parse().expect("number");
        let d = svg.split(" d=\"").nth(1).expect("path");
        let (d, _) = d.split_once('"').expect("end of path");
        let mut dark = vec![false; n * n];
        for run in d.split_terminator('z') {
            // `M{x} {y}h{len}v1h-{len}`: one row high.
            let run = run.strip_prefix('M').expect("move");
            let (x, run) = run.split_once(' ').expect("x");
            let (y, run) = run.split_once('h').expect("y");
            let (len, run) = run.split_once("v1h-").expect("len");
            assert_eq!(run, len);
            let (x, y, len): (usize, usize, usize) = (
                x.parse().expect("x"),
                y.parse().expect("y"),
                len.parse().expect("len"),
            );
            for cell in &mut dark[y * n + x..y * n + x + len] {
                assert!(!*cell, "runs overlap");
                *cell = true;
            }
        }
        (n, dark)
    }

    /// Every code found in the modules of `svg`, drawn 4 pixels per module with a quiet zone of 4
    /// modules.
    fn decode(svg: &str) -> Vec<String> {
        const SCALE: usize = 4;
        const QUIET: usize = 4;
        let (n, dark) = modules(svg);
        let side = (n + 2 * QUIET) * SCALE;
        let mut image = rqrr::PreparedImage::prepare_from_greyscale(side, side, |x, y| {
            let (x, y) = (x / SCALE, y / SCALE);
            let inside = (QUIET..QUIET + n).contains(&x) && (QUIET..QUIET + n).contains(&y);
            if inside && dark[(y - QUIET) * n + x - QUIET] {
                0
            } else {
                255
            }
        });
        image
            .detect_grids()
            .iter()
            .map(|grid| grid.decode().expect("decodes").1)
            .collect()
    }

    /// Cyrillic whose UTF-8 bytes look like Shift-JIS pairs (capitals А–П, small с–я).
    const KANJI_LOOKALIKES: [&str; 3] = [
        "https://example.com/o/ИВАНОВ",
        "https://example.com/?q=стыдсчёттуфхцчшщъыьэюя",
        "https://example.com/?q=ДЕВАЙС ПОДМЕНА",
    ];

    /// `text` as one byte-mode segment in a symbol of `version` (level M), built here on its own;
    /// `None` when it does not fit.
    fn byte_symbol(text: &str, version: i16) -> Option<QrCode> {
        let mut bits = Bits::new(Version::Normal(version));
        bits.push_byte_data(text.as_bytes()).ok()?;
        bits.push_terminator(EcLevel::M).ok()?;
        QrCode::with_bits(bits, EcLevel::M).ok()
    }

    fn dark(code: &QrCode) -> Vec<bool> {
        code.to_colors().iter().map(|c| *c == Color::Dark).collect()
    }

    #[test]
    fn svg_draws_the_modules_of_the_symbol() {
        let svg = qr_svg(TELEGRAM).expect("fits");
        let (n, modules_drawn) = modules(&svg);
        assert_eq!((n - 21) % 4, 0, "{n}");
        assert!(svg.contains(&format!("viewBox=\"0 0 {n} {n}\"")), "{svg}");
        assert!(svg.contains("shape-rendering=\"crispEdges\""));
        assert_eq!(svg.matches("<path").count(), 1);
        let version = i16::try_from((n - 17) / 4).expect("version");
        let code = byte_symbol(TELEGRAM, version).expect("fits");
        assert_eq!(code.width(), n);
        assert_eq!(modules_drawn, dark(&code));
    }

    /// Every text is one byte-mode segment in the smallest symbol that holds it: no Kanji segment,
    /// which phones would read as Shift-JIS, and no other mode either.
    #[test]
    fn every_text_is_one_byte_segment() {
        let digits = "https://example.com/o/12345678901234567890";
        let other: Vec<&str> = [TELEGRAM, digits]
            .into_iter()
            .chain(KANJI_LOOKALIKES)
            .filter(|text| {
                let (n, modules_drawn) = modules(&qr_svg(text).expect("fits"));
                let version = i16::try_from((n - 17) / 4).expect("version");
                let smallest = version == 1 || byte_symbol(text, version - 1).is_none();
                byte_symbol(text, version).map(|code| dark(&code)) != Some(modules_drawn)
                    || !smallest
            })
            .collect();
        assert!(other.is_empty(), "not one byte segment: {other:?}");
    }

    #[test]
    fn svg_reads_back_as_the_text() {
        for text in [
            TELEGRAM,
            "https://t.me/сервис_пример",
            "https://yandex.ru/maps/?text=Энск",
        ]
        .into_iter()
        .chain(KANJI_LOOKALIKES)
        {
            assert_eq!(decode(&qr_svg(text).expect("fits")), [text]);
        }
    }

    #[test]
    fn text_too_long_for_a_symbol_has_none() {
        assert!(qr_svg(&"a".repeat(3000)).is_none());
    }

    /// [`MAX_BYTES`] is what the largest symbol holds: a byte more fits no version.
    #[test]
    fn largest_symbol_holds_max_bytes() {
        let fits = "a".repeat(MAX_BYTES);
        assert!(byte_symbol(&fits, 40).is_some());
        assert!(byte_symbol(&format!("{fits}a"), 40).is_none());
        let (n, _) = modules(&qr_svg(&fits).expect("fits"));
        assert_eq!(n, 177);
        assert!(qr_svg(&format!("{fits}a")).is_none());
        // Bytes, not characters: a Cyrillic letter is two.
        assert!(qr_svg(&"я".repeat(MAX_BYTES / 2)).is_some());
        assert!(qr_svg(&"я".repeat(MAX_BYTES / 2 + 1)).is_none());
    }

    #[test]
    fn empty_text_does_not_panic() {
        let _ = qr_svg("");
    }
}
