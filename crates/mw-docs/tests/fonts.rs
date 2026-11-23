use std::collections::BTreeSet;
use std::path::Path;

use mw_docs::{
    FontSet, FontSources, RenderOptions, compile_with_fonts_for_tests, render_with_fonts,
    sample_data, standard_template,
};
use mw_domain::documents::DocumentKind;
use typst::text::{FontInfo, FontStyle, FontWeight};

/// Full block: not in Libertinus Serif, present in DejaVu Sans Mono.
const BLOCK: char = '\u{2588}';
const MONO: &str = "DejaVu Sans Mono";

fn dejavu_mono() -> &'static [u8] {
    typst_assets::fonts()
        .find(|data| {
            FontInfo::new(data, 0).is_some_and(|i| {
                i.family == MONO
                    && i.variant.weight == FontWeight::REGULAR
                    && i.variant.style == FontStyle::Normal
            })
        })
        .unwrap_or_else(|| panic!("DejaVu Sans Mono is bundled with typst-assets"))
}

fn folder_with(files: &[(&str, &[u8])]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("temp dir: {e:?}"));
    for (name, bytes) in files {
        std::fs::write(dir.path().join(name), bytes)
            .unwrap_or_else(|e| panic!("write file: {e:?}"));
    }
    dir
}

fn from_dir(dir: &Path) -> FontSet {
    FontSet::new(&FontSources {
        dirs: vec![dir.to_path_buf()],
        system: false,
    })
}

fn families(set: &FontSet, text: &str) -> BTreeSet<String> {
    compile_with_fonts_for_tests(format!("Текст {text}"), vec![], set)
        .unwrap_or_else(|e| panic!("document compiles: {e:?}"))
        .font_families()
}

#[test]
fn fallback_font_from_dir_prints_missing_glyph() {
    let text = BLOCK.to_string();
    let embedded = FontSet::new(&FontSources::default());
    assert_eq!(
        embedded.missing(&text),
        vec![BLOCK],
        "the sign must be missing in Libertinus"
    );
    assert!(!families(&embedded, &text).contains(MONO));

    let dir = folder_with(&[("DejaVuSansMono.ttf", dejavu_mono())]);
    let with_dir = from_dir(dir.path());
    assert!(with_dir.missing(&text).is_empty());
    let used = families(&with_dir, &text);
    assert!(used.contains(MONO), "{used:?}");
    assert!(used.contains("Libertinus Serif"), "{used:?}");
}

#[test]
fn broken_font_file_is_skipped() {
    let junk: Vec<u8> = (0..100u32).map(|i| (i * 37 % 251) as u8).collect();
    let dir = folder_with(&[
        ("broken.ttf", &junk),
        ("notes.txt", b"not a font"),
        ("DejaVuSansMono.ttf", dejavu_mono()),
    ]);
    let set = from_dir(dir.path());
    assert!(set.missing("Текст").is_empty());
    assert!(families(&set, &BLOCK.to_string()).contains(MONO));
}

#[test]
fn font_file_that_vanished_is_not_a_panic() {
    let dir = folder_with(&[("DejaVuSansMono.ttf", dejavu_mono())]);
    let set = from_dir(dir.path());
    std::fs::remove_file(dir.path().join("DejaVuSansMono.ttf"))
        .unwrap_or_else(|e| panic!("remove: {e:?}"));
    // The face is in the book but its file is gone: the document is built, the glyph is a stub.
    let used = families(&set, &BLOCK.to_string());
    assert!(!used.contains(MONO), "{used:?}");
}

#[test]
fn missing_glyph_everywhere_does_not_panic() {
    let dir = folder_with(&[("DejaVuSansMono.ttf", dejavu_mono())]);
    let set = from_dir(dir.path());
    let text = '\u{10FFFD}'.to_string();
    assert_eq!(set.missing(&text), vec!['\u{10FFFD}']);
    let doc = compile_with_fonts_for_tests(format!("Текст {text}"), vec![], &set)
        .unwrap_or_else(|e| panic!("builds: {e:?}"));
    assert_eq!(doc.page_count(), 1);
}

#[test]
fn covered_text_looks_the_same() {
    let dir = folder_with(&[("DejaVuSansMono.ttf", dejavu_mono())]);
    let plain = FontSet::new(&FontSources::default());
    let extended = from_dir(dir.path());
    let kind = DocumentKind::IntakeAct;
    let png = |fonts: &FontSet| {
        render_with_fonts(
            &standard_template(kind),
            &sample_data(kind),
            &RenderOptions::default(),
            fonts,
        )
        .unwrap_or_else(|e| panic!("renders: {e:?}"))
        .png_pages(96)
        .unwrap_or_else(|e| panic!("png: {e:?}"))
    };
    assert_eq!(png(&plain), png(&extended));
}
