//! Fonts of a document. Libertinus Serif (SIL OFL 1.1, see `fonts/OFL.txt`) is embedded and always first,
//! so a document looks the same on every machine as long as its text is covered by it. The faces that
//! follow serve only the glyphs Libertinus lacks (Typst's own fallback picks a face that has the glyph):
//! first the fonts folder of the program's data, then the system fonts.
//!
//! The book (`FontInfo` of every face) is built once; the font itself is read from its file on first use.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use fontdb::Source;
use typst::foundations::Bytes;
use typst::text::{Font, FontBook, FontInfo};
use typst::utils::LazyHash;

const FACES: [&[u8]; 4] = [
    include_bytes!("../fonts/LibertinusSerif-Regular.otf"),
    include_bytes!("../fonts/LibertinusSerif-Italic.otf"),
    include_bytes!("../fonts/LibertinusSerif-Semibold.otf"),
    include_bytes!("../fonts/LibertinusSerif-Bold.otf"),
];

/// Where fallback fonts come from, besides the embedded ones.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FontSources {
    /// Folders with font files, in order of preference.
    pub dirs: Vec<PathBuf>,
    /// Whether to add the fonts installed in the system (after the folders).
    pub system: bool,
}

/// A face of the set: an embedded one is ready, one from a file is read on first use.
struct Slot {
    file: Option<(PathBuf, u32)>,
    font: OnceLock<Option<Font>>,
}

impl Slot {
    fn ready(font: Font) -> Self {
        Self {
            file: None,
            font: OnceLock::from(Some(font)),
        }
    }

    fn lazy(path: PathBuf, index: u32) -> Self {
        Self {
            file: Some((path, index)),
            font: OnceLock::new(),
        }
    }

    /// `None` when the file has vanished or no longer holds the face.
    fn get(&self) -> Option<Font> {
        self.font
            .get_or_init(|| {
                let (path, index) = self.file.as_ref()?;
                let data = std::fs::read(path).ok()?;
                Font::new(Bytes::new(data), *index)
            })
            .clone()
    }
}

/// The faces a document may use and the book Typst chooses from.
pub struct FontSet {
    book: LazyHash<FontBook>,
    slots: Vec<Slot>,
}

impl FontSet {
    /// Embedded faces first, then the folders, then the system. An unreadable or broken file is skipped.
    pub fn new(sources: &FontSources) -> Self {
        let mut infos = Vec::new();
        let mut slots = Vec::new();
        for font in FACES.iter().flat_map(|data| Font::iter(Bytes::new(*data))) {
            infos.push(font.info().clone());
            slots.push(Slot::ready(font));
        }
        let mut db = fontdb::Database::new();
        for dir in &sources.dirs {
            db.load_fonts_dir(dir);
        }
        if sources.system {
            db.load_system_fonts();
        }
        let mut seen = HashSet::new();
        // The faces of one file follow each other, so the file is read once for all of them.
        let mut loaded: Option<(PathBuf, Option<Vec<u8>>)> = None;
        for face in db.faces() {
            let (Source::File(path) | Source::SharedFile(path, _)) = &face.source else {
                continue;
            };
            if !seen.insert((path.clone(), face.index)) {
                continue;
            }
            if loaded.as_ref().is_none_or(|(p, _)| p != path) {
                loaded = Some((path.clone(), std::fs::read(path).ok()));
            }
            let Some(info) = loaded
                .as_ref()
                .and_then(|(_, data)| data.as_deref())
                .and_then(|data| FontInfo::new(data, face.index))
            else {
                continue;
            };
            infos.push(info);
            slots.push(Slot::lazy(path.clone(), face.index));
        }
        Self {
            book: LazyHash::new(FontBook::from_infos(infos)),
            slots,
        }
    }

    pub(crate) fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    pub(crate) fn font(&self, index: usize) -> Option<Font> {
        self.slots.get(index)?.get()
    }

    /// Characters of `text` that no face of the set has a glyph for (each once, in order of appearance).
    pub fn missing(&self, text: &str) -> Vec<char> {
        let mut missing = Vec::new();
        for c in text.chars() {
            if c.is_whitespace() || c.is_control() || missing.contains(&c) {
                continue;
            }
            let covered = (0..self.slots.len()).any(|i| {
                self.book
                    .info(i)
                    .is_some_and(|f| f.coverage.contains(c as u32))
            });
            if !covered {
                missing.push(c);
            }
        }
        missing
    }
}

struct Shared {
    sources: FontSources,
    /// The set is being or has been built: sources can no longer change.
    frozen: bool,
}

static SHARED: Mutex<Shared> = Mutex::new(Shared {
    sources: FontSources {
        dirs: Vec::new(),
        system: false,
    },
    frozen: false,
});
static SET: OnceLock<FontSet> = OnceLock::new();

/// Remembers where fallback fonts come from (cheap; call it before the first document). A call after the
/// shared set was built changes nothing. Without a call the set holds the embedded fonts only.
pub fn configure_fonts(sources: FontSources) {
    let mut shared = SHARED.lock().unwrap_or_else(|e| e.into_inner());
    if shared.frozen {
        tracing::warn!("шрифты уже загружены: источники запасных шрифтов не изменены");
        return;
    }
    shared.sources = sources;
}

/// Builds the shared set now, so that the first document does not wait for it.
pub fn warm_fonts() {
    shared();
}

pub(crate) fn shared() -> &'static FontSet {
    SET.get_or_init(|| {
        let sources = {
            let mut shared = SHARED.lock().unwrap_or_else(|e| e.into_inner());
            shared.frozen = true;
            shared.sources.clone()
        };
        FontSet::new(&sources)
    })
}

/// Characters of `text` that no face of the shared set has a glyph for (each listed once, in order of
/// appearance).
pub fn font_covers(text: &str) -> Vec<char> {
    shared().missing(text)
}
