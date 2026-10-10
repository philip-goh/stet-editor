//! Undo/redo history for a [`TextBuffer`].

use crate::edit::AppliedEdit;
use crate::text::TextBuffer;

/// An undo/redo stack of applied edits.
///
/// The history stores [`AppliedEdit`]s, which carry both the removed and the
/// inserted text, so undo and redo are plain buffer edits and never need a
/// snapshot of the whole document.
///
/// Consecutive single-character insertions (typing) are coalesced into one
/// undo step. Deletions and multi-character edits are kept as individual
/// steps for now; grouping them like real editors do is tracked in the
/// roadmap.
#[derive(Clone, Debug, Default)]
pub struct History {
    undo_stack: Vec<AppliedEdit>,
    redo_stack: Vec<AppliedEdit>,
}

impl History {
    /// Creates an empty history.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if there is an edit to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Returns `true` if there is an edit to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Records an edit that was already applied to a buffer.
    ///
    /// Recording any edit clears the redo stack, mirroring what every text
    /// editor does after a divergent edit.
    pub fn record(&mut self, edit: AppliedEdit) {
        if self.can_coalesce(&edit) {
            if let Some(last) = self.undo_stack.last_mut() {
                last.coalesce_with(edit);
            }
        } else {
            self.undo_stack.push(edit);
        }
        self.redo_stack.clear();
    }

    fn can_coalesce(&self, next: &AppliedEdit) -> bool {
        self.undo_stack
            .last()
            .is_some_and(|last| last.can_coalesce(next))
    }

    /// Undoes the most recent edit and returns the edit that was applied to
    /// the buffer.
    pub fn undo(&mut self, buffer: &mut TextBuffer) -> Option<AppliedEdit> {
        let edit = self.undo_stack.pop()?;
        let inverse = buffer.edit(
            edit.range.start..edit.range.start + edit.inserted.len(),
            &edit.removed,
        );
        self.redo_stack.push(edit);
        Some(inverse)
    }

    /// Redoes the most recently undone edit and returns it.
    pub fn redo(&mut self, buffer: &mut TextBuffer) -> Option<AppliedEdit> {
        let edit = self.redo_stack.pop()?;
        let redo_start = edit.range.start;
        let redo_end = redo_start + edit.removed.len();
        let applied = buffer.edit(redo_start..redo_end, &edit.inserted);
        self.undo_stack.push(edit);
        Some(applied)
    }

    /// Clears both stacks.
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_redo_roundtrip() {
        let mut buffer = TextBuffer::new("hello");
        let mut history = History::new();
        history.record(buffer.edit(5..5, " world"));
        assert_eq!(buffer.text(), "hello world");

        history.undo(&mut buffer);
        assert_eq!(buffer.text(), "hello");
        history.undo(&mut buffer);
        assert_eq!(buffer.text(), "hello");

        history.redo(&mut buffer);
        assert_eq!(buffer.text(), "hello world");
        history.redo(&mut buffer);
        assert_eq!(buffer.text(), "hello world");
    }

    #[test]
    fn typing_coalesces_into_one_undo_step() {
        let mut buffer = TextBuffer::new("");
        let mut history = History::new();
        history.record(buffer.edit(0..0, "a"));
        history.record(buffer.edit(1..1, "b"));
        history.record(buffer.edit(2..2, "c"));
        assert_eq!(buffer.text(), "abc");

        history.undo(&mut buffer);
        assert_eq!(buffer.text(), "");
        assert!(!history.can_undo());
    }

    #[test]
    fn multi_character_insertions_do_not_coalesce() {
        // Typing should coalesce, but a paste (multi-char insert) should be
        // its own undo step even though it is adjacent.
        let mut buffer = TextBuffer::new("");
        let mut history = History::new();
        history.record(buffer.edit(0..0, "a"));
        history.record(buffer.edit(1..1, "bc"));
        assert_eq!(buffer.text(), "abc");

        history.undo(&mut buffer);
        assert_eq!(buffer.text(), "a");
    }

    #[test]
    fn deletions_are_separate_steps() {
        let mut buffer = TextBuffer::new("abc");
        let mut history = History::new();
        history.record(buffer.edit(2..3, ""));
        history.record(buffer.edit(1..2, ""));
        assert_eq!(buffer.text(), "a");

        history.undo(&mut buffer);
        assert_eq!(buffer.text(), "ab");
        history.undo(&mut buffer);
        assert_eq!(buffer.text(), "abc");
    }

    #[test]
    fn new_edit_clears_redo_stack() {
        let mut buffer = TextBuffer::new("a");
        let mut history = History::new();
        history.record(buffer.edit(1..1, "b"));
        history.undo(&mut buffer);
        assert!(history.can_redo());

        history.record(buffer.edit(1..1, "c"));
        assert!(!history.can_redo());
        history.redo(&mut buffer);
        assert_eq!(buffer.text(), "ac");
    }

    #[test]
    fn undo_after_coalesced_typing_matches_original() {
        let mut buffer = TextBuffer::new("x");
        let mut history = History::new();
        history.record(buffer.edit(1..1, "a"));
        history.record(buffer.edit(2..2, "b"));
        history.record(buffer.edit(3..3, "c"));
        assert_eq!(buffer.text(), "xabc");
        history.undo(&mut buffer);
        assert_eq!(buffer.text(), "x");
        history.redo(&mut buffer);
        assert_eq!(buffer.text(), "xabc");
    }
}
