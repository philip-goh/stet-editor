//! File-backed documents: a text buffer plus path, dirty state and history.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::edit::AppliedEdit;
use crate::error::{Error, Result};
use crate::history::History;
use crate::text::TextBuffer;

/// A text buffer with file identity, dirty tracking and undo/redo.
///
/// Dirty state is revision-based: every edit, undo or redo bumps a
/// revision counter, and saving records the revision that was written.
/// A consequence is that undoing back to exactly the saved content still
/// reports dirty; content-hash based cleanliness is a roadmap item.
#[derive(Debug)]
pub struct Document {
    buffer: TextBuffer,
    history: History,
    path: Option<PathBuf>,
    revision: u64,
    saved_revision: u64,
}

impl Document {
    /// Creates an empty, untitled document.
    pub fn untitled() -> Self {
        Self::from_text("")
    }

    /// Creates an untitled document with initial text.
    pub fn from_text(text: impl AsRef<str>) -> Self {
        Self {
            buffer: TextBuffer::new(text),
            history: History::new(),
            path: None,
            revision: 0,
            saved_revision: 0,
        }
    }

    /// Opens the file at `path`.
    ///
    /// A UTF-8 byte order mark is stripped. Invalid UTF-8 is rejected with
    /// [`Error::NotUtf8`]; encoding detection is a roadmap item.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let bytes = std::fs::read(&path).map_err(|source| Error::Read {
            path: path.clone(),
            source,
        })?;
        let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
        let text = std::str::from_utf8(bytes).map_err(|_| Error::NotUtf8 { path: path.clone() })?;
        Ok(Self {
            buffer: TextBuffer::new(text),
            history: History::new(),
            path: Some(path),
            revision: 1,
            saved_revision: 1,
        })
    }

    /// Returns the document's path, if it has one.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Returns the file name, or `untitled` for documents without a path.
    pub fn display_name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "untitled".to_owned())
    }

    /// Returns the buffer.
    pub fn buffer(&self) -> &TextBuffer {
        &self.buffer
    }

    /// Returns `true` if there are unsaved changes.
    pub fn is_dirty(&self) -> bool {
        self.revision != self.saved_revision
    }

    /// Applies an edit and records it in the history.
    pub fn edit(&mut self, range: std::ops::Range<usize>, text: &str) -> AppliedEdit {
        let edit = self.buffer.edit(range, text);
        self.history.record(edit.clone());
        self.revision += 1;
        edit
    }

    /// Undoes the most recent edit, if any.
    pub fn undo(&mut self) -> Option<AppliedEdit> {
        let edit = self.history.undo(&mut self.buffer)?;
        self.revision += 1;
        Some(edit)
    }

    /// Redoes the most recently undone edit, if any.
    pub fn redo(&mut self) -> Option<AppliedEdit> {
        let edit = self.history.redo(&mut self.buffer)?;
        self.revision += 1;
        Some(edit)
    }

    /// Saves the document to its current path.
    ///
    /// Returns [`Error::NoPath`] for untitled documents; use
    /// [`Document::save_as`] to give the document a path.
    pub fn save(&mut self) -> Result<()> {
        let path = self.path.clone().ok_or(Error::NoPath)?;
        self.write_to(&path)?;
        Ok(())
    }

    /// Saves the document to `path` and adopts it as the document's path.
    pub fn save_as(&mut self, path: impl Into<PathBuf>) -> Result<()> {
        let path = path.into();
        self.write_to(&path)?;
        self.path = Some(path);
        Ok(())
    }

    /// Reloads the document from disk, discarding history and changes.
    pub fn reload(&mut self) -> Result<()> {
        let path = self.path.clone().ok_or(Error::NoPath)?;
        let reopened = Self::open(path)?;
        self.buffer = reopened.buffer;
        self.history = History::new();
        self.revision += 1;
        self.saved_revision = self.revision;
        Ok(())
    }

    /// Writes atomically: the new content goes to a temporary file in the
    /// same directory, which is then renamed over the target. This prevents
    /// half-written files if the editor crashes mid-save.
    fn write_to(&mut self, path: &Path) -> Result<()> {
        let map_err = |source| Error::Write {
            path: path.to_path_buf(),
            source,
        };
        let directory = match path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent,
            _ => Path::new("."),
        };
        let mut temporary = tempfile::NamedTempFile::new_in(directory).map_err(map_err)?;
        temporary
            .write_all(self.buffer.text().as_bytes())
            .map_err(map_err)?;
        temporary.flush().map_err(map_err)?;
        temporary.persist(path).map_err(|error| Error::Write {
            path: path.to_path_buf(),
            source: error.error,
        })?;
        self.saved_revision = self.revision;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn untitled_documents_are_clean_until_edited() {
        let mut document = Document::untitled();
        assert!(!document.is_dirty());
        assert_eq!(document.display_name(), "untitled");
        document.edit(0..0, "hello");
        assert!(document.is_dirty());
        assert!(document.save().is_err());
    }

    #[test]
    fn open_save_roundtrip() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("notes.txt");
        std::fs::write(&path, "first line\nsecond line\n").unwrap();

        let mut document = Document::open(&path).unwrap();
        assert_eq!(document.display_name(), "notes.txt");
        assert!(!document.is_dirty());
        assert_eq!(document.buffer().line_count(), 3);

        document.edit(0..5, "1st");
        assert!(document.is_dirty());
        document.save().unwrap();
        assert!(!document.is_dirty());
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "1st line\nsecond line\n"
        );
    }

    #[test]
    fn save_as_adopts_new_path() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("new.txt");
        let mut document = Document::from_text("content");
        document.edit(7..7, "!");
        document.save_as(&path).unwrap();
        assert_eq!(document.path(), Some(path.as_path()));
        assert!(!document.is_dirty());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "content!");
    }

    #[test]
    fn strips_utf8_bom() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("bom.txt");
        std::fs::write(&path, b"\xEF\xBB\xBFhello").unwrap();
        let document = Document::open(&path).unwrap();
        assert_eq!(document.buffer().text(), "hello");
    }

    #[test]
    fn rejects_invalid_utf8() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("binary.bin");
        std::fs::write(&path, [0xFF, 0xFE, 0x00]).unwrap();
        assert!(matches!(Document::open(&path), Err(Error::NotUtf8 { .. })));
    }

    #[test]
    fn undo_and_redo_bump_revision() {
        let mut document = Document::from_text("abc");
        document.edit(3..3, "d");
        document.undo();
        document.redo();
        assert_eq!(document.buffer().text(), "abcd");
        assert!(document.is_dirty());
    }

    #[test]
    fn reload_discards_changes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("reload.txt");
        std::fs::write(&path, "original").unwrap();
        let mut document = Document::open(&path).unwrap();
        document.edit(0..8, "changed");
        assert!(document.is_dirty());
        document.reload().unwrap();
        assert_eq!(document.buffer().text(), "original");
        assert!(!document.is_dirty());
    }
}
