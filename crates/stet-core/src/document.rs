//! File-backed documents: a text buffer plus path, dirty state and history.

use std::io::Write;
use std::ops::Range;
use std::path::{Path, PathBuf};

use crate::edit::AppliedEdit;
use crate::error::{Error, Result};
use crate::history::{History, UndoStep};
use crate::text::TextBuffer;

/// A text buffer with file identity, dirty tracking and undo/redo.
///
/// Dirty state is content-hash based: a document is clean exactly when its
/// current text hashes to what was last loaded or saved, so undoing back to
/// the saved content correctly clears the dirty marker.
#[derive(Debug)]
pub struct Document {
    buffer: TextBuffer,
    history: History,
    path: Option<PathBuf>,
    content_hash: u64,
    saved_hash: u64,
}

impl Document {
    /// Creates an empty, untitled document.
    pub fn untitled() -> Self {
        Self::from_text("")
    }

    /// Creates an untitled document with initial text.
    pub fn from_text(text: impl AsRef<str>) -> Self {
        let buffer = TextBuffer::new(text);
        let hash = buffer.content_hash();
        Self {
            buffer,
            history: History::new(),
            path: None,
            content_hash: hash,
            saved_hash: hash,
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
        let buffer = TextBuffer::new(text);
        let hash = buffer.content_hash();
        Ok(Self {
            buffer,
            history: History::new(),
            path: Some(path),
            content_hash: hash,
            saved_hash: hash,
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
        self.content_hash != self.saved_hash
    }

    /// Returns `true` if there is an edit to undo.
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    /// Returns `true` if there is an edit to redo.
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// Applies an edit and records it in the history.
    pub fn edit(&mut self, range: Range<usize>, text: &str) -> AppliedEdit {
        let edit = self.buffer.edit(range, text);
        self.history.record(edit.clone());
        self.content_hash = self.buffer.content_hash();
        edit
    }

    /// Applies several edits as one undo step.
    ///
    /// All ranges are in the coordinate space of the buffer *before* the
    /// batch; edits are applied right to left so they do not shift each
    /// other. Manual ordering by the caller is not required.
    pub fn edit_batch(
        &mut self,
        edits: impl IntoIterator<Item = (Range<usize>, String)>,
    ) -> Vec<AppliedEdit> {
        let mut pending: Vec<(Range<usize>, String)> = edits.into_iter().collect();
        pending.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
        let applied: Vec<AppliedEdit> = pending
            .into_iter()
            .map(|(range, text)| self.buffer.edit(range, &text))
            .collect();
        self.history.record_step(applied.clone());
        self.content_hash = self.buffer.content_hash();
        applied
    }

    /// Undoes the most recent step, if any.
    ///
    /// Returns the undone step; use [`UndoStep::start`] to place the caret.
    pub fn undo(&mut self) -> Option<UndoStep> {
        let step = self.history.undo(&mut self.buffer)?;
        self.content_hash = self.buffer.content_hash();
        Some(step)
    }

    /// Redoes the most recently undone step, if any.
    pub fn redo(&mut self) -> Option<UndoStep> {
        let step = self.history.redo(&mut self.buffer)?;
        self.content_hash = self.buffer.content_hash();
        Some(step)
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
        self.content_hash = reopened.content_hash;
        self.saved_hash = reopened.saved_hash;
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
        self.saved_hash = self.content_hash;
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
    fn undoing_back_to_saved_content_is_clean() {
        let mut document = Document::from_text("abc");
        document.edit(3..3, "d");
        assert!(document.is_dirty());
        document.undo();
        assert!(!document.is_dirty());
        document.redo();
        assert!(document.is_dirty());
    }

    #[test]
    fn editing_elsewhere_is_still_dirty() {
        let mut document = Document::from_text("abc");
        document.edit(0..0, "x");
        document.undo();
        assert!(!document.is_dirty());
        // A different edit that happens to restore the same text is clean;
        // a different text is dirty.
        document.edit(0..1, "z");
        assert!(document.is_dirty());
    }

    #[test]
    fn edit_batch_applies_as_one_step() {
        let mut document = Document::from_text("one\ntwo\n");
        document.edit_batch(vec![(0..0, "  ".to_owned()), (4..4, "  ".to_owned())]);
        assert_eq!(document.buffer().text(), "  one\n  two\n");
        assert!(document.can_undo());

        document.undo();
        assert_eq!(document.buffer().text(), "one\ntwo\n");
        assert!(!document.can_undo());
        assert!(!document.is_dirty());
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
