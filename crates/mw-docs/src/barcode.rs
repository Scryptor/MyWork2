//! UPC-A and Code 128 barcodes as SVG: bars only. The human-readable text is printed by Typst.

use std::fmt::Write;

use mw_domain::barcode::is_valid_upc_a;

/// Left-half digit patterns (odd parity); right-half patterns are their complements.
const L_CODES: [&str; 10] = [
    "0001101", "0011001", "0010011", "0111101", "0100011", "0110001", "0101111", "0111011",
    "0110111", "0001011",
];

const QUIET_ZONE_MODULES: usize = 9;
const BAR_MODULES: usize = 95;
/// Modules of a whole UPC-A symbol, the quiet zones included.
pub(crate) const UPC_A_MODULES: usize = BAR_MODULES + 2 * QUIET_ZONE_MODULES;
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

/// The module for a code of `modules` modules (the quiet zones included) that has `available_mm` of
/// width: [`module_mm`] when the whole symbol fits; otherwise the widest that fits. With DPI it stays a whole number of
/// dots, never fewer than 2 (or than [`module_mm`] itself, when that is 1 dot); without DPI it is
/// never narrower than 0.25 mm. A code that does not fit even then is printed at that minimum.
pub(crate) fn fit_module_mm(available_mm: f64, modules: usize, dpi: Option<u32>) -> f64 {
    let widest = available_mm / modules as f64;
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

/// Like [`fit_module_mm`] (same `modules`), but the module also grows: the widest that fits `available_mm` (with DPI a
/// whole number of dots), never narrower than [`fit_module_mm`] gives.
pub(crate) fn stretch_module_mm(available_mm: f64, modules: usize, dpi: Option<u32>) -> f64 {
    let fitted = fit_module_mm(available_mm, modules, dpi);
    let widest = available_mm / modules as f64;
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

/// The symbology a value is printed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Symbology {
    UpcA,
    Code128,
}

/// UPC-A when `value` is a valid one (order codes of MyWork 3 and most old ones); otherwise Code 128
/// when it is 1 to 40 printable ASCII characters; otherwise `None`: such a value prints no barcode.
pub fn symbology(value: &str) -> Option<Symbology> {
    if is_valid_upc_a(value) {
        Some(Symbology::UpcA)
    } else {
        code128_symbols(value).map(|_| Symbology::Code128)
    }
}

/// The size of a printed symbol: its whole width in modules (the quiet zones included) and how far
/// the guard bars stick out below the regular ones, in modules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Shape {
    pub(crate) modules: usize,
    guard_modules: usize,
}

impl Shape {
    pub(crate) const UPC_A: Self = Self {
        modules: UPC_A_MODULES,
        guard_modules: GUARD_EXTENSION_MODULES,
    };

    /// The shape of the symbol `value` is printed as; `None` when it prints no barcode.
    pub(crate) fn of(value: &str) -> Option<Self> {
        match symbology(value)? {
            Symbology::UpcA => Some(Self::UPC_A),
            Symbology::Code128 => Some(Self::of_code128(code128_symbols(value)?.len())),
        }
    }

    /// Whether the whole symbol fits `width_mm` at the narrowest module (see [`fit_module_mm`]).
    pub(crate) fn fits(self, width_mm: f64, dpi: Option<u32>) -> bool {
        self.modules as f64 * fit_module_mm(width_mm, self.modules, dpi) <= width_mm + 1e-6
    }

    /// A Code 128 code of `symbols` symbols, start, check and stop included.
    fn of_code128(symbols: usize) -> Self {
        Self {
            modules: 11 * symbols + 2 + 2 * CODE128_QUIET_MODULES,
            guard_modules: 0,
        }
    }

    /// Size (width, height) in mm of the SVG of this shape; `height_mm` is the height of the regular bars.
    pub(crate) fn size_mm(self, module_mm: f64, height_mm: f64) -> (f64, f64) {
        (
            self.modules as f64 * module_mm,
            height_mm + self.guard_modules as f64 * module_mm,
        )
    }
}

#[cfg(test)]
fn shape(value: &str) -> Option<Shape> {
    Shape::of(value)
}

/// SVG of `value` in whichever symbology [`symbology`] picks, or `None` when it has none.
pub(crate) fn barcode_svg(value: &str, module_mm: f64, height_mm: f64) -> Option<String> {
    match symbology(value)? {
        Symbology::UpcA => upc_a_svg(value, module_mm, height_mm),
        Symbology::Code128 => code128_svg(value, module_mm, height_mm),
    }
}

/// SVG of a UPC-A code in millimetres: black `<rect>` bars (adjacent modules merged), no text.
/// `height_mm` is the height of regular bars; guard bars are 5 modules longer. The width is
/// 113 (95 + 2 * 9) modules, the quiet zone included. `None` for an invalid code.
pub fn upc_a_svg(code: &str, module_mm: f64, height_mm: f64) -> Option<String> {
    let bits = modules(code)?;
    let (width, guard_height) = Shape::UPC_A.size_mm(module_mm, height_mm);
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

const CODE128_QUIET_MODULES: usize = 10;
const CODE128_MAX_CHARS: usize = 40;
const START_B: u8 = 104;
const START_C: u8 = 105;
const STOP: u8 = 106;

/// Bar and space widths of every Code 128 symbol: values 0 to 105, then the stop.
const PATTERNS: [&str; 107] = [
    "212222", "222122", "222221", "121223", "121322", "131222", "122213", "122312", "132212",
    "221213", "221312", "231212", "112232", "122132", "122231", "113222", "123122", "123221",
    "223211", "221132", "221231", "213212", "223112", "312131", "311222", "321122", "321221",
    "312212", "322112", "322211", "212123", "212321", "232121", "111323", "131123", "131321",
    "112313", "132113", "132311", "211313", "231113", "231311", "112133", "112331", "132131",
    "113123", "113321", "133121", "313121", "211331", "231131", "213113", "213311", "213131",
    "311123", "311321", "331121", "312113", "312311", "332111", "314111", "221411", "431111",
    "111224", "111422", "121124", "121421", "141122", "141221", "112214", "112412", "122114",
    "122411", "142112", "142211", "241211", "221114", "413111", "241112", "134111", "111242",
    "121142", "121241", "114212", "124112", "124211", "411212", "421112", "421211", "212141",
    "214121", "412121", "111143", "111341", "131141", "114113", "114311", "411113", "411311",
    "113141", "114131", "311141", "411131", "211412", "211214", "211232", "2331112",
];

/// The symbol values of a Code 128 code for `value`: start, data, check, stop. A value of an even
/// number of digits, at least four, goes in set C (two digits a symbol), any other in set B (one character a
/// symbol). `None` for an empty value, one longer than 40 characters or one with anything but
/// printable ASCII.
pub(crate) fn code128_symbols(value: &str) -> Option<Vec<u8>> {
    let bytes = value.as_bytes();
    if bytes.is_empty()
        || bytes.len() > CODE128_MAX_CHARS
        || !bytes.iter().all(|b| (b' '..=b'~').contains(b))
    {
        return None;
    }
    let mut symbols = Vec::with_capacity(bytes.len() + 3);
    if bytes.len() >= 4 && bytes.len().is_multiple_of(2) && bytes.iter().all(u8::is_ascii_digit) {
        symbols.push(START_C);
        symbols.extend(
            bytes
                .chunks(2)
                .map(|pair| (pair[0] - b'0') * 10 + (pair[1] - b'0')),
        );
    } else {
        symbols.push(START_B);
        symbols.extend(bytes.iter().map(|b| b - b' '));
    }
    let sum: usize = usize::from(symbols[0])
        + symbols
            .iter()
            .enumerate()
            .skip(1)
            .map(|(position, &value)| position * usize::from(value))
            .sum::<usize>();
    symbols.push(u8::try_from(sum % 103).ok()?);
    symbols.push(STOP);
    Some(symbols)
}

/// SVG of a Code 128 code in millimetres: black `<rect>` bars all `height_mm` high, no text. The width
/// is 11 modules a symbol (start, data and check) and 13 for the stop, plus a quiet zone of 10 modules
/// on each side. `None` when [`code128_symbols`] has none.
pub fn code128_svg(value: &str, module_mm: f64, height_mm: f64) -> Option<String> {
    let symbols = code128_symbols(value)?;
    let (width, height) = Shape::of_code128(symbols.len()).size_mm(module_mm, height_mm);
    let mut svg = String::new();
    let _ = write!(
        svg,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width:.4}mm\" height=\"{height:.4}mm\" \
         viewBox=\"0 0 {width:.4} {height:.4}\" shape-rendering=\"crispEdges\">"
    );
    let mut x = CODE128_QUIET_MODULES as f64 * module_mm;
    for &symbol in &symbols {
        for (i, w) in PATTERNS[usize::from(symbol)].bytes().enumerate() {
            let w = f64::from(w - b'0') * module_mm;
            if i % 2 == 0 {
                let _ = write!(
                    svg,
                    "<rect x=\"{x:.4}\" y=\"0\" width=\"{w:.4}\" height=\"{height:.4}\" fill=\"#000\"/>"
                );
            }
            x += w;
        }
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
        assert!((fit_module_mm(37.0, UPC_A_MODULES, Some(203)) - 2.0 * dot_203).abs() < 1e-9);
        assert!((fit_module_mm(37.0, UPC_A_MODULES, Some(203)) - 0.2502).abs() < 1e-4);
        // 58×40: 55 mm is enough for the usual 3 dots, and wider never makes the module wider.
        assert!((fit_module_mm(55.0, UPC_A_MODULES, Some(203)) - 3.0 * dot_203).abs() < 1e-9);
        assert_eq!(
            fit_module_mm(500.0, UPC_A_MODULES, Some(203)),
            module_mm(Some(203))
        );
        // Never fewer than 2 dots, however narrow.
        assert!((fit_module_mm(10.0, UPC_A_MODULES, Some(203)) - 2.0 * dot_203).abs() < 1e-9);
        assert!((fit_module_mm(10.0, UPC_A_MODULES, Some(600)) - 2.0 * 25.4 / 600.0).abs() < 1e-9);
        // 72 dpi: one dot is already more than 0.33 mm, so it stays one dot.
        assert_eq!(
            fit_module_mm(10.0, UPC_A_MODULES, Some(72)),
            module_mm(Some(72))
        );
        // Without DPI: continuous, 0.33 mm at most, 0.25 mm at least.
        assert_eq!(fit_module_mm(55.0, UPC_A_MODULES, None), 0.33);
        assert!((fit_module_mm(33.9, UPC_A_MODULES, None) - 0.3).abs() < 1e-9);
        assert_eq!(fit_module_mm(10.0, UPC_A_MODULES, None), 0.25);
        assert_eq!(fit_module_mm(f64::NAN, UPC_A_MODULES, None), 0.33);
        // The symbol with its quiet zones fits whenever it can.
        for (width, dpi) in [(37.0, Some(203)), (40.0, Some(300)), (37.0, None)] {
            assert!(
                Shape::UPC_A
                    .size_mm(fit_module_mm(width, UPC_A_MODULES, dpi), 10.0)
                    .0
                    <= width
            );
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

    #[test]
    fn symbology_by_value() {
        assert_eq!(symbology("036000291452"), Some(Symbology::UpcA));
        assert_eq!(symbology("036000291453"), Some(Symbology::Code128));
        assert_eq!(symbology("MW-1"), Some(Symbology::Code128));
        assert_eq!(symbology("Заказ"), None);
        assert_eq!(symbology("a\nb"), None);
        assert_eq!(symbology(&"7".repeat(40)), Some(Symbology::Code128));
        assert_eq!(symbology(&"7".repeat(41)), None);
        assert_eq!(symbology(""), None);
    }

    #[test]
    fn code128_symbols_b_and_c() {
        assert_eq!(code128_symbols("A"), Some(vec![104, 33, 34, 106]));
        assert_eq!(code128_symbols("1234"), Some(vec![105, 12, 34, 82, 106]));
        assert_eq!(code128_symbols("123"), Some(vec![104, 17, 18, 19, 8, 106]));
    }

    #[test]
    fn code128_patterns_are_well_formed() {
        let distinct: std::collections::HashSet<_> = PATTERNS.iter().collect();
        assert_eq!(PATTERNS.len(), 107);
        assert_eq!(distinct.len(), 107);
        let widths = |p: &str| -> Vec<u32> { p.bytes().map(|b| u32::from(b - b'0')).collect() };
        for (value, pattern) in PATTERNS[..106].iter().enumerate() {
            let w = widths(pattern);
            assert_eq!(w.len(), 6, "{value}");
            assert_eq!(w.iter().sum::<u32>(), 11, "{value}");
            // Every code character has an even total width of bars: the format's own parity check.
            assert_eq!(w.iter().step_by(2).sum::<u32>() % 2, 0, "{value}");
        }
        assert_eq!(PATTERNS[106], "2331112");
        assert_eq!(widths(PATTERNS[106]).iter().sum::<u32>(), 13);
        assert_eq!(PATTERNS[103], "211412");
        assert_eq!(PATTERNS[104], "211214");
        assert_eq!(PATTERNS[105], "211232");
        assert_eq!(PATTERNS[0], "212222");
    }

    /// Reads the symbol values back from the bars of a Code 128 SVG.
    fn decode(svg: &str, module: f64) -> Vec<u8> {
        let total = (svg_width(svg) / module).round() as usize;
        let mut bits = vec![b'0'; total];
        for (x, w, _) in rects(svg) {
            let first = (x / module).round() as usize;
            for b in &mut bits[first..first + (w / module).round() as usize] {
                *b = b'1';
            }
        }
        let bits = &bits[CODE128_QUIET_MODULES..total - CODE128_QUIET_MODULES];
        let runs = |chunk: &[u8]| -> String {
            let mut out = String::new();
            let mut i = 0;
            while i < chunk.len() {
                let start = i;
                while i < chunk.len() && chunk[i] == chunk[start] {
                    i += 1;
                }
                out.push_str(&(i - start).to_string());
            }
            out
        };
        let (body, stop) = bits.split_at(bits.len() - 13);
        let mut values: Vec<u8> = body
            .chunks(11)
            .map(|c| PATTERNS.iter().position(|p| *p == runs(c)).expect("known") as u8)
            .collect();
        assert_eq!(runs(stop), PATTERNS[106]);
        values.push(106);
        values
    }

    fn svg_width(svg: &str) -> f64 {
        let s = svg.find("width=\"").unwrap() + 7;
        svg[s..s + svg[s..].find("mm\"").unwrap()].parse().unwrap()
    }

    #[test]
    fn code128_svg_round_trips() {
        let m = 0.3754;
        for value in ["MW-1", "1234", "A-1053", "036000291453"] {
            let svg = code128_svg(value, m, 12.0).expect("encodable");
            assert_eq!(decode(&svg, m), code128_symbols(value).unwrap(), "{value}");
            assert!(rects(&svg).iter().all(|r| (r.2 - 12.0).abs() < 1e-3));
            assert!(!svg.contains("<text"));
        }
        assert!(code128_svg("Заказ", m, 12.0).is_none());
    }

    #[test]
    fn code128_width_is_whole_dots() {
        let module = module_mm(Some(203));
        assert!((module - 3.0 * 25.4 / 203.0).abs() < 1e-9);
        for value in ["A", "MW-1", "A-1053", "1234"] {
            let symbols = code128_symbols(value).unwrap().len();
            let svg = code128_svg(value, module, 10.0).unwrap();
            let expected = (11 * symbols + 2 + 20) as f64 * module;
            assert!((svg_width(&svg) - expected).abs() < 1e-3, "{value}");
            assert_eq!(
                shape(value).unwrap().size_mm(module, 10.0),
                (expected, 10.0)
            );
        }
    }
}
