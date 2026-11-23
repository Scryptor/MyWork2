//! UPC-A barcode as SVG: bars only. The human-readable digits are printed by Typst.

use std::fmt::Write;

use mw_domain::barcode::is_valid_upc_a;

/// Left-half digit patterns (odd parity); right-half patterns are their complements.
const L_CODES: [&str; 10] = [
    "0001101", "0011001", "0010011", "0111101", "0100011", "0110001", "0101111", "0111011",
    "0110111", "0001011",
];

const QUIET_ZONE_MODULES: usize = 9;
const BAR_MODULES: usize = 95;
/// Guard bars extend this many modules below the regular bars.
const GUARD_EXTENSION_MODULES: usize = 5;
/// Default module width without printer DPI, mm.
const DEFAULT_MODULE_MM: f64 = 0.33;
/// The narrowest module a code shrinks to when it does not fit: without DPI in mm, with DPI in
/// dots.
const MIN_MODULE_MM: f64 = 0.25;
const MIN_MODULE_DOTS: f64 = 2.0;

/// Module width in mm. Without DPI it is 0.33 mm; with DPI it is a whole number of printer dots,
/// `ceil(0.33 mm * dpi / 25.4)`, converted back to mm (203 dpi: 3 dots = 0.3754 mm).
pub fn module_mm(dpi: Option<u32>) -> f64 {
    match dpi {
        Some(dpi) if dpi > 0 => {
            let dots = (DEFAULT_MODULE_MM * f64::from(dpi) / 25.4).ceil().max(1.0);
            dots * 25.4 / f64::from(dpi)
        }
        _ => DEFAULT_MODULE_MM,
    }
}

/// The module for a code that has `available_mm` of width: [`module_mm`] when the whole symbol,
/// quiet zones included, fits; otherwise the widest that fits. With DPI it stays a whole number of
/// dots, never fewer than 2 (or than [`module_mm`] itself, when that is 1 dot); without DPI it is
/// never narrower than 0.25 mm. A code that does not fit even then is printed at that minimum.
pub(crate) fn fit_module_mm(available_mm: f64, dpi: Option<u32>) -> f64 {
    let widest = available_mm / (BAR_MODULES + 2 * QUIET_ZONE_MODULES) as f64;
    match dpi {
        Some(dpi) if dpi > 0 => {
            let dot = 25.4 / f64::from(dpi);
            let snapped = (module_mm(Some(dpi)) / dot).round();
            // A hair of tolerance: 2 dots must fit exactly 2 dots' worth of width.
            let fitting = (widest / dot + 1e-9).floor();
            fitting.min(snapped).max(MIN_MODULE_DOTS.min(snapped)) * dot
        }
        // A broken template (a non-finite page) gets the usual module.
        _ if widest.is_nan() => DEFAULT_MODULE_MM,
        _ => widest.clamp(MIN_MODULE_MM, DEFAULT_MODULE_MM),
    }
}

/// Like [`fit_module_mm`], but the module also grows: the widest that fits `available_mm` (with DPI a
/// whole number of dots), never narrower than [`fit_module_mm`] gives.
pub(crate) fn stretch_module_mm(available_mm: f64, dpi: Option<u32>) -> f64 {
    let fitted = fit_module_mm(available_mm, dpi);
    let widest = available_mm / (BAR_MODULES + 2 * QUIET_ZONE_MODULES) as f64;
    match dpi {
        Some(dpi) if dpi > 0 => {
            let dot = 25.4 / f64::from(dpi);
            fitted.max((widest / dot + 1e-9).floor() * dot)
        }
        _ => fitted.max(widest),
    }
}

/// The 95 modules of a UPC-A code (`true` = bar): guard 101, six left digits, guard 01010,
/// six right digits, guard 101. `None` unless `code` is a valid UPC-A.
fn modules(code: &str) -> Option<[bool; BAR_MODULES]> {
    if !is_valid_upc_a(code) {
        return None;
    }
    let mut bits = String::with_capacity(BAR_MODULES);
    bits.push_str("101");
    for (i, digit) in code.bytes().enumerate() {
        if i == 6 {
            bits.push_str("01010");
        }
        let pattern = L_CODES[usize::from(digit - b'0')];
        if i < 6 {
            bits.push_str(pattern);
        } else {
            bits.extend(pattern.chars().map(|c| if c == '0' { '1' } else { '0' }));
        }
    }
    bits.push_str("101");
    let mut out = [false; BAR_MODULES];
    for (slot, c) in out.iter_mut().zip(bits.chars()) {
        *slot = c == '1';
    }
    Some(out)
}

/// Size (width, height) in mm of the SVG that [`upc_a_svg`] makes.
pub(crate) fn upc_a_size_mm(module_mm: f64, height_mm: f64) -> (f64, f64) {
    (
        (BAR_MODULES + 2 * QUIET_ZONE_MODULES) as f64 * module_mm,
        height_mm + GUARD_EXTENSION_MODULES as f64 * module_mm,
    )
}

/// SVG of a UPC-A code in millimetres: black `<rect>` bars (adjacent modules merged), no text.
/// `height_mm` is the height of regular bars; guard bars are 5 modules longer. The width is
/// (95 + 2 * 9) modules, the quiet zone included. `None` for an invalid code.
pub fn upc_a_svg(code: &str, module_mm: f64, height_mm: f64) -> Option<String> {
    let bits = modules(code)?;
    let (width, guard_height) = upc_a_size_mm(module_mm, height_mm);
    let mut svg = String::new();
    let _ = write!(
        svg,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width:.4}mm\" height=\"{guard_height:.4}mm\" \
         viewBox=\"0 0 {width:.4} {guard_height:.4}\" shape-rendering=\"crispEdges\">"
    );
    let mut i = 0;
    while i < BAR_MODULES {
        if !bits[i] {
            i += 1;
            continue;
        }
        let start = i;
        while i < BAR_MODULES && bits[i] {
            i += 1;
        }
        let is_guard = start < 3 || (45..50).contains(&start) || start >= BAR_MODULES - 3;
        let height = if is_guard { guard_height } else { height_mm };
        let x = (QUIET_ZONE_MODULES + start) as f64 * module_mm;
        let w = (i - start) as f64 * module_mm;
        let _ = write!(
            svg,
            "<rect x=\"{x:.4}\" y=\"0\" width=\"{w:.4}\" height=\"{height:.4}\" fill=\"#000\"/>"
        );
    }
    svg.push_str("</svg>");
    Some(svg)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPECTED: &str = "10100011010111101010111100011010001101000110101010110110011101001100110101110010011101101100101";

    #[test]
    fn modules_of_036000291452() {
        let bits = modules("036000291452").expect("valid");
        let s: String = bits.iter().map(|&b| if b { '1' } else { '0' }).collect();
        assert_eq!(s, EXPECTED);
        assert_eq!(s.len(), 95);
    }

    #[test]
    fn invalid_codes_give_none() {
        assert!(upc_a_svg("036000291453", 0.33, 15.0).is_none());
        assert!(upc_a_svg("03600029145", 0.33, 15.0).is_none());
        assert!(upc_a_svg("abc", 0.33, 15.0).is_none());
    }

    /// Parses `(x, width, height)` of every rect.
    fn rects(svg: &str) -> Vec<(f64, f64, f64)> {
        let attr = |tag: &str, name: &str| -> f64 {
            let key = format!("{name}=\"");
            let s = tag.find(&key).unwrap() + key.len();
            tag[s..s + tag[s..].find('"').unwrap()].parse().unwrap()
        };
        svg.split("<rect")
            .skip(1)
            .map(|t| (attr(t, "x"), attr(t, "width"), attr(t, "height")))
            .collect()
    }

    #[test]
    fn svg_geometry_matches_modules() {
        let m = 0.33;
        let svg = upc_a_svg("036000291452", m, 15.0).expect("valid");
        assert!(svg.contains("shape-rendering=\"crispEdges\""));
        assert!(svg.contains(&format!("width=\"{:.4}mm\"", 113.0 * m)));
        assert!(!svg.contains("<text"));
        // Reconstruct the module string from the rects.
        let mut bits = [b'0'; 95];
        for (x, w, _) in rects(&svg) {
            let first = (x / m).round() as usize - QUIET_ZONE_MODULES;
            for b in &mut bits[first..first + (w / m).round() as usize] {
                *b = b'1';
            }
        }
        assert_eq!(std::str::from_utf8(&bits).unwrap(), EXPECTED);
        // Guards are 5 modules taller than the regular bars.
        let heights: Vec<f64> = rects(&svg).iter().map(|r| r.2).collect();
        assert!((heights[0] - (15.0 + 5.0 * m)).abs() < 1e-3);
        assert!((heights[3] - 15.0).abs() < 1e-3);
        // Adjacent modules are merged: far fewer rects than set modules.
        assert!(rects(&svg).len() < EXPECTED.matches('1').count());
    }

    #[test]
    fn module_shrinks_to_the_available_width() {
        let dot_203 = 25.4 / 203.0;
        // A 40×30 sticker with 1.5 mm margins: 3 dots (42.4 mm) do not fit 37 mm, 2 dots do.
        assert!((fit_module_mm(37.0, Some(203)) - 2.0 * dot_203).abs() < 1e-9);
        assert!((fit_module_mm(37.0, Some(203)) - 0.2502).abs() < 1e-4);
        // 58×40: 55 mm is enough for the usual 3 dots, and wider never makes the module wider.
        assert!((fit_module_mm(55.0, Some(203)) - 3.0 * dot_203).abs() < 1e-9);
        assert_eq!(fit_module_mm(500.0, Some(203)), module_mm(Some(203)));
        // Never fewer than 2 dots, however narrow.
        assert!((fit_module_mm(10.0, Some(203)) - 2.0 * dot_203).abs() < 1e-9);
        assert!((fit_module_mm(10.0, Some(600)) - 2.0 * 25.4 / 600.0).abs() < 1e-9);
        // 72 dpi: one dot is already more than 0.33 mm, so it stays one dot.
        assert_eq!(fit_module_mm(10.0, Some(72)), module_mm(Some(72)));
        // Without DPI: continuous, 0.33 mm at most, 0.25 mm at least.
        assert_eq!(fit_module_mm(55.0, None), 0.33);
        assert!((fit_module_mm(33.9, None) - 0.3).abs() < 1e-9);
        assert_eq!(fit_module_mm(10.0, None), 0.25);
        assert_eq!(fit_module_mm(f64::NAN, None), 0.33);
        // The symbol with its quiet zones fits whenever it can.
        for (width, dpi) in [(37.0, Some(203)), (40.0, Some(300)), (37.0, None)] {
            assert!(upc_a_size_mm(fit_module_mm(width, dpi), 10.0).0 <= width);
        }
    }

    #[test]
    fn module_width_by_dpi() {
        assert_eq!(module_mm(None), 0.33);
        let dots =
            |dpi: u32, n: f64| (module_mm(Some(dpi)) - n * 25.4 / f64::from(dpi)).abs() < 1e-9;
        assert!(dots(203, 3.0));
        assert!((module_mm(Some(203)) - 0.3754).abs() < 1e-4);
        assert!(dots(300, 4.0));
        assert!((module_mm(Some(300)) - 0.33867).abs() < 1e-5);
        assert!(dots(600, 8.0));
        assert!((module_mm(Some(600)) - 0.33867).abs() < 1e-5);
    }
}
