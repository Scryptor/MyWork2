//! QR codes as SVG: the dark modules only. The room around the code is left by the document.

use std::fmt::Write;

use qrcode::{Color, EcLevel, QrCode};

/// SVG of `text` (its UTF-8 bytes) as a QR code of error correction level M: one unit per module
/// (`viewBox="0 0 n n"`, `n` the side of the symbol in modules), no quiet zone, one `<path>` of the
/// runs of dark modules row by row. `None` when the text does not fit a QR code.
pub(crate) fn qr_svg(text: &str) -> Option<String> {
    let code = QrCode::with_error_correction_level(text.as_bytes(), EcLevel::M).ok()?;
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

    #[test]
    fn svg_draws_the_modules_of_the_symbol() {
        let svg = qr_svg(TELEGRAM).expect("fits");
        let code =
            QrCode::with_error_correction_level(TELEGRAM.as_bytes(), EcLevel::M).expect("fits");
        let n = code.width();
        assert_eq!((n - 21) % 4, 0, "{n}");
        assert!(svg.contains(&format!("viewBox=\"0 0 {n} {n}\"")), "{svg}");
        assert!(svg.contains("shape-rendering=\"crispEdges\""));
        assert_eq!(svg.matches("<path").count(), 1);
        let expected: Vec<bool> = code.to_colors().iter().map(|c| *c == Color::Dark).collect();
        assert_eq!(modules(&svg), (n, expected));
    }

    #[test]
    fn svg_reads_back_as_the_text() {
        for text in [
            TELEGRAM,
            "https://t.me/сервис_пример",
            "https://yandex.ru/maps/?text=Энск",
        ] {
            assert_eq!(decode(&qr_svg(text).expect("fits")), [text]);
        }
    }

    #[test]
    fn text_too_long_for_a_symbol_has_none() {
        assert!(qr_svg(&"a".repeat(3000)).is_none());
    }

    #[test]
    fn empty_text_does_not_panic() {
        let _ = qr_svg("");
    }
}
