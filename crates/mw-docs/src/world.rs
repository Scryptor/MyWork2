//! The Typst `World`: the main file, a few more virtual sources and files and a set of fonts. Nothing
//! else exists for the document: no disk, no packages, no current date.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};

use crate::fonts::FontSet;
use crate::render::DocsError;

fn library() -> &'static LazyHash<Library> {
    static LIBRARY: OnceLock<LazyHash<Library>> = OnceLock::new();
    LIBRARY.get_or_init(|| LazyHash::new(Library::default()))
}

pub(crate) fn file_id(path: &str) -> Option<FileId> {
    let vpath = VirtualPath::new(path).ok()?;
    Some(RootedPath::new(VirtualRoot::Project, vpath).intern())
}

pub(crate) struct DocsWorld<'a> {
    fonts: &'a FontSet,
    main: FileId,
    /// Typst sources by id, the main one included.
    sources: HashMap<FileId, Source>,
    files: HashMap<FileId, Bytes>,
}

impl<'a> DocsWorld<'a> {
    /// `main` is `/main.typ`; `sources` are more Typst sources and `files` other files, by path.
    pub(crate) fn new(
        main: String,
        sources: Vec<(String, String)>,
        files: Vec<(String, Vec<u8>)>,
        fonts: &'a FontSet,
    ) -> Result<Self, DocsError> {
        let main_id =
            file_id("/main.typ").unwrap_or_else(|| unreachable!("constant path is valid"));
        let id_of = |name: String| file_id(&name).ok_or(DocsError::BadFileName(name));
        let mut by_id = HashMap::from([(main_id, Source::new(main_id, main))]);
        for (name, text) in sources {
            let id = id_of(name)?;
            by_id.insert(id, Source::new(id, text));
        }
        let files = files
            .into_iter()
            .map(|(name, bytes)| Ok((id_of(name)?, Bytes::new(bytes))))
            .collect::<Result<_, DocsError>>()?;
        Ok(Self {
            fonts,
            main: main_id,
            sources: by_id,
            files,
        })
    }
}

fn not_found(id: FileId) -> FileError {
    FileError::NotFound(PathBuf::from(id.vpath().get_without_slash()))
}

impl World for DocsWorld<'_> {
    fn library(&self) -> &LazyHash<Library> {
        library()
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.fonts.book()
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        self.sources.get(&id).cloned().ok_or_else(|| not_found(id))
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if let Some(source) = self.sources.get(&id) {
            return Ok(Bytes::from_string(source.text().to_owned()));
        }
        self.files.get(&id).cloned().ok_or_else(|| not_found(id))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.font(index)
    }

    /// Documents never depend on the current date: templates get dates from the data.
    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}
