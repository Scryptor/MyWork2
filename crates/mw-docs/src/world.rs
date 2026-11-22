//! The Typst `World`: one main file, a few virtual files and the embedded fonts.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};

use crate::fonts::fonts;
use crate::render::DocsError;

fn library() -> &'static LazyHash<Library> {
    static LIBRARY: OnceLock<LazyHash<Library>> = OnceLock::new();
    LIBRARY.get_or_init(|| LazyHash::new(Library::default()))
}

fn file_id(path: &str) -> Option<FileId> {
    let vpath = VirtualPath::new(path).ok()?;
    Some(RootedPath::new(VirtualRoot::Project, vpath).intern())
}

pub(crate) struct DocsWorld {
    main: Source,
    files: HashMap<FileId, Bytes>,
}

impl DocsWorld {
    pub(crate) fn new(source: String, files: Vec<(String, Vec<u8>)>) -> Result<Self, DocsError> {
        let main_id =
            file_id("/main.typ").unwrap_or_else(|| unreachable!("constant path is valid"));
        let mut map = HashMap::new();
        for (name, bytes) in files {
            let id = file_id(&name).ok_or_else(|| DocsError::BadFileName(name.clone()))?;
            map.insert(id, Bytes::new(bytes));
        }
        Ok(Self {
            main: Source::new(main_id, source),
            files: map,
        })
    }
}

impl World for DocsWorld {
    fn library(&self) -> &LazyHash<Library> {
        library()
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &fonts().book
    }

    fn main(&self) -> FileId {
        self.main.id()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main.id() {
            Ok(self.main.clone())
        } else {
            Err(FileError::NotFound(PathBuf::from(
                id.vpath().get_without_slash(),
            )))
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.main.id() {
            return Ok(Bytes::from_string(self.main.text().to_owned()));
        }
        self.files
            .get(&id)
            .cloned()
            .ok_or_else(|| FileError::NotFound(PathBuf::from(id.vpath().get_without_slash())))
    }

    fn font(&self, index: usize) -> Option<Font> {
        fonts().fonts.get(index).cloned()
    }

    /// Documents never depend on the current date: templates get dates from the data.
    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}
