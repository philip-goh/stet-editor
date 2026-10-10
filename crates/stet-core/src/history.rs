//! Undo/redo history for a [`TextBuffer`].
//!
//! History is a stack of **steps**, where a step is one or more
//! [`AppliedEdit`]s that undo together. Single edits (typing, deleting) are
//! recorded one at a time; batched operations (block indent, multi-cursor
//! typing, replace-all) record a transaction so they undo as one unit.
//!
//! Consecutive single-grapheme insertions and deletions are coalesced into
//! the most recent step. Multi-character edits — pastes, word/line kills —
//! stay atomic.

use crate::edit::AppliedEdit;
use crate::text::TextBuffer;

/// One undoable unit: edits in application order.
///
/// Undo applies the inverses in reverse order; redo re-applies them
/// forward. Both are correct because each edit's range refers to the buffer
/// state just before that edit was applied.
#[derive(Clone, Debug, Default)]
pub struct UndoStep {
    edits: Vec<AppliedEdit>,
    /// Whether this step is a run of single-grapheme edits that may still
    /// absorb further typing or backspacing.
    open_run: bool,
}

impl UndoStep {
    /// Creates a step with a single edit.
    pub fn single(edit: AppliedEdit) -> Self {
        let open_run = edit.is_single_grapheme_edit();
        Self {
            edits: vec![edit],
            open_run,
        }
    }

    /// Creates a step from edits in application order.
    ///
    /// Transactions are always closed: they never absorb later edits.
    pub fn from_edits(edits: Vec<AppliedEdit>) -> Self {
        Self {
            edits,
            open_run: false,
        }
    }

    /// Returns `true` if the step contains no edits.
    pub fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }

    /// Returns the edits in application order.
    pub fn edits(&self) -> &[AppliedEdit] {
        &self.edits
    }

    /// Returns the smallest edited byte offset, for caret placement after
    /// an undo.
    pub fn start(&self) -> usize {
        self.edits
            .iter()
            .map(|edit| edit.range.start)
            .min()
            .unwrap_or(0)
    }

    fn can_accept(&self, next: &AppliedEdit) -> bool {
        self.open_run
            && next.is_single_grapheme_edit()
            && self
                .edits
                .last()
                .is_some_and(|last| last.can_coalesce(next))
    }

    fn merge(&mut self, edit: AppliedEdit) {
        if let Some(last) = self.edits.last_mut() {
            last.coalesce_with(edit);
        }
    }
}

/// An undo/redo stack of [`UndoStep`]s.
#[derive(Clone, Debug, Default)]
pub struct History {
    undo_stack: Vec<UndoStep>,
    redo_stack: Vec<UndoStep>,
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
        if let Some(last) = self.undo_stack.last_mut()
            && last.can_accept(&edit)
        {
            last.merge(edit);
        } else {
            self.undo_stack.push(UndoStep::single(edit));
        }
        self.redo_stack.clear();
    }

    /// Records a transaction of edits, applied in the given order, as one
    /// undo step.
    pub fn record_step(&mut self, edits: Vec<AppliedEdit>) {
        if edits.is_empty() {
            return;
        }
        self.undo_stack.push(UndoStep::from_edits(edits));
        self.redo_stack.clear();
    }

    /// Undoes the most recent step and returns it.
    pub fn undo(&mut self, buffer: &mut TextBuffer) -> Option<UndoStep> {
        let step = self.undo_stack.pop()?;
        for edit in step.edits().iter().rev() {
            buffer.edit(
                edit.range.start..edit.range.start + edit.inserted.len(),
                &edit.removed,
            );
        }
        self.redo_stack.push(step.clone());
        Some(step)
    }

    /// Redoes the most recently undone step and returns it.
    pub fn redo(&mut self, buffer: &mut TextBuffer) -> Option<UndoStep> {
        let step = self.redo_stack.pop()?;
        for edit in step.edits() {
            buffer.edit(
                edit.range.start..edit.range.start + edit.removed.len(),
                &edit.inserted,
            );
        }
        self.undo_stack.push(step.clone());
        Some(step)
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
    fn backspace_run_coalesces_into_one_undo_step() {
        let mut buffer = TextBuffer::new("abc");
        let mut history = History::new();
        // Three backspaces at the end of the buffer.
        history.record(buffer.edit(2..3, ""));
        history.record(buffer.edit(1..2, ""));
        history.record(buffer.edit(0..1, ""));
        assert_eq!(buffer.text(), "");

        history.undo(&mut buffer);
        assert_eq!(buffer.text(), "abc");
        assert!(!history.can_undo());
    }

    #[test]
    fn forward_delete_run_coalesces_into_one_undo_step() {
        let mut buffer = TextBuffer::new("abcd");
        let mut history = History::new();
        // Delete key pressed twice at offset 1.
        history.record(buffer.edit(1..2, ""));
        history.record(buffer.edit(1..2, ""));
        assert_eq!(buffer.text(), "ad");

        history.undo(&mut buffer);
        assert_eq!(buffer.text(), "abcd");
        assert!(!history.can_undo());
    }

    #[test]
    fn multi_character_deletions_stay_atomic() {
        let mut buffer = TextBuffer::new("one two");
        let mut history = History::new();
        // Delete "two", then the space: two distinct steps.
        history.record(buffer.edit(4..7, ""));
        history.record(buffer.edit(3..4, ""));
        assert_eq!(buffer.text(), "one");

        history.undo(&mut buffer);
        assert_eq!(buffer.text(), "one ");
        history.undo(&mut buffer);
        assert_eq!(buffer.text(), "one two");
    }

    #[test]
    fn typing_then_deleting_are_separate_steps() {
        let mut buffer = TextBuffer::new("");
        let mut history = History::new();
        history.record(buffer.edit(0..0, "a"));
        history.record(buffer.edit(1..1, "b"));
        history.record(buffer.edit(1..2, ""));
        assert_eq!(buffer.text(), "a");

        history.undo(&mut buffer);
        assert_eq!(buffer.text(), "ab");
        history.undo(&mut buffer);
        assert_eq!(buffer.text(), "");
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

    #[test]
    fn transactions_undo_as_one_step() {
        let mut buffer = TextBuffer::new("one\ntwo\n");
        let mut history = History::new();
        // Indent both lines with two spaces, applied right-to-left.
        let second = buffer.edit(4..4, "  ");
        let first = buffer.edit(0..0, "  ");
        history.record_step(vec![second, first]);
        assert_eq!(buffer.text(), "  one\n  two\n");

        history.undo(&mut buffer);
        assert_eq!(buffer.text(), "one\ntwo\n");
        assert!(!history.can_undo());

        history.redo(&mut buffer);
        assert_eq!(buffer.text(), "  one\n  two\n");
    }
}
