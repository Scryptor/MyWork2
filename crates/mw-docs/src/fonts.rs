//! Embedded fonts: Libertinus Serif (SIL OFL 1.1, see `fonts/OFL.txt`). No system fonts are ever read,
//! so a document looks the same on every machine.

use std::sync::OnceLock;

use typst::foundations::Bytes;
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;

const FACES: [&[u8]; 4] = [
    include_bytes!("../fonts/LibertinusSerif-Regular.otf"),
    include_bytes!("../fonts/LibertinusSerif-Italic.otf"),
    include_bytes!("../fonts/LibertinusSerif-Semibold.otf"),
    include_bytes!("../fonts/LibertinusSerif-Bold.otf"),
];

pub(crate) struct Fonts {
    pub(crate) book: LazyHash<FontBook>,
    pub(crate) fonts: Vec<Font>,
}

pub(crate) fn fonts() -> &'static Fonts {
    static FONTS: OnceLock<Fonts> = OnceLock::new();
    FONTS.get_or_init(|| {
        let fonts: Vec<Font> = FACES
            .iter()
            .flat_map(|data| Font::iter(Bytes::new(*data)))
            .collect();
        let book = FontBook::from_infos(fonts.iter().map(|f| f.info().clone()));
        Fonts {
            book: LazyHash::new(book),
            fonts,
        }
    })
}

/// Characters of `text` that no embedded face has a glyph for (each listed once, in order of appearance).
pub fn font_covers(text: &str) -> Vec<char> {
    let fonts = fonts();
    let mut missing = Vec::new();
    for c in text.chars() {
        if c.is_whitespace() || c.is_control() || missing.contains(&c) {
            continue;
        }
        if !fonts
            .fonts
            .iter()
            .any(|f| f.info().coverage.contains(c as u32))
        {
            missing.push(c);
        }
    }
    missing
}
