//! Edits applied to a [`TextBuffer`](crate::TextBuffer) and the incremental
//! information consumers need to react to them.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

/// A row/column point in bytes, mirroring `tree_sitter::Point`.
///
/// Syntax highlighting needs to tell tree-sitter where an edit happened so
/// it can incrementally reparse. Tree-sitter columns are measured in
/// **bytes**, unlike [`Position`](crate::Position) which measures chars.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Point {
    /// Zero-based row (line) index.
    pub row: usize,
    /// Zero-based byte column within the row.
    pub column: usize,
}

impl Point {
    /// Creates a new point.
    pub const fn new(row: usize, column: usize) -> Self {
        Self { row, column }
    }
}

/// The kind of edit that was applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditKind {
    /// Text was inserted and nothing was removed.
    Insert,
    /// Text was removed and nothing was inserted.
    Delete,
    /// Text was replaced with other text.
    Replace,
}

/// Incremental parse information for a single edit, in tree-sitter terms.
///
/// All byte offsets refer to the buffer *before* the edit except
/// `new_end_byte` and `new_end_position`, which refer to the buffer *after*.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditSummary {
    /// Byte offset where the edit started.
    pub start_byte: usize,
    /// Byte offset where the replaced range ended before the edit.
    pub old_end_byte: usize,
    /// Byte offset where the inserted text ends after the edit.
    pub new_end_byte: usize,
    /// Line/byte-column where the edit started.
    pub start_position: Point,
    /// Line/byte-column where the replaced range ended before the edit.
    pub old_end_position: Point,
    /// Line/byte-column where the inserted text ends after the edit.
    pub new_end_position: Point,
}

/// A concrete edit that was applied to a buffer.
///
/// Records everything needed to undo the edit (`range` + `removed`) and to
/// redo it (`inserted`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppliedEdit {
    /// The byte range that was replaced, in pre-edit coordinates.
    pub range: Range<usize>,
    /// The text that was inserted.
    pub inserted: String,
    /// The text that was removed.
    pub removed: String,
    /// How the edit should be classified.
    pub kind: EditKind,
    /// Incremental parse information.
    pub summary: EditSummary,
}

impl AppliedEdit {
    /// Returns `true` if this edit inserts text without removing any.
    pub fn is_insert(&self) -> bool {
        self.kind == EditKind::Insert
    }

    /// Returns `true` if this edit removes text without inserting any.
    pub fn is_delete(&self) -> bool {
        self.kind == EditKind::Delete
    }

    /// Returns the byte range of the inserted text in post-edit
    /// coordinates.
    pub fn inserted_range(&self) -> Range<usize> {
        self.range.start..self.range.start + self.inserted.len()
    }

    /// Returns `true` if this edit is a single-grapheme insertion or
    /// deletion, i.e. eligible to be part of a coalescing run.
    pub(crate) fn is_single_grapheme_edit(&self) -> bool {
        match self.kind {
            EditKind::Insert => self.removed.is_empty() && is_single_grapheme(&self.inserted),
            EditKind::Delete => self.inserted.is_empty() && is_single_grapheme(&self.removed),
            EditKind::Replace => false,
        }
    }

    /// Returns `true` if `self` and `next` are adjacent edits of the same
    /// kind that may collapse into one undo step.
    ///
    /// Whether a step is *open* to further coalescing is tracked by the
    /// history; this only checks the geometry.
    pub(crate) fn can_coalesce(&self, next: &AppliedEdit) -> bool {
        match (self.kind, next.kind) {
            (EditKind::Insert, EditKind::Insert) => {
                self.removed.is_empty()
                    && next.removed.is_empty()
                    && self.range.start + self.inserted.len() == next.range.start
            }
            (EditKind::Delete, EditKind::Delete) => {
                self.inserted.is_empty()
                    && next.inserted.is_empty()
                    // Backward run (backspace): the newer edit sits directly
                    // before the step. Forward run (delete): both edits start
                    // at the same offset.
                    && (next.range.end == self.range.start
                        || next.range.start == self.range.start)
            }
            _ => false,
        }
    }

    pub(crate) fn coalesce_with(&mut self, next: AppliedEdit) {
        match (self.kind, next.kind) {
            (EditKind::Insert, EditKind::Insert) => {
                self.inserted.push_str(&next.inserted);
                self.summary.new_end_byte = next.summary.new_end_byte;
                self.summary.new_end_position = next.summary.new_end_position;
            }
            (EditKind::Delete, EditKind::Delete) => {
                if next.range.end == self.range.start {
                    // Backward run: prepend the newer deletion.
                    self.range.start = next.range.start;
                    self.removed = next.removed + &self.removed;
                    self.summary.start_byte = next.summary.start_byte;
                    self.summary.start_position = next.summary.start_position;
                } else {
                    // Forward run: append the newer deletion.
                    self.range.end = next.range.end;
                    self.removed.push_str(&next.removed);
                    self.summary.old_end_byte = next.summary.old_end_byte;
                    self.summary.old_end_position = next.summary.old_end_position;
                }
            }
            _ => {}
        }
    }
}

fn is_single_grapheme(text: &str) -> bool {
    text.graphemes(true).count() == 1
}
