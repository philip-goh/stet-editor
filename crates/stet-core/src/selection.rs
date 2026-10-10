//! Selections as anchor/head pairs of byte offsets.

/// A selection expressed as an `anchor` and a `head` byte offset.
///
/// The head is where the caret is; the anchor is where the selection was
/// started. When they are equal the selection is collapsed at the caret.
/// Keeping both (instead of a sorted range) preserves the direction of the
/// selection, which matters for shift+arrow behaviour and for rendering the
/// caret at the moving end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    /// The byte offset where the selection started.
    pub anchor: usize,
    /// The byte offset of the caret (the moving end).
    pub head: usize,
}

impl Selection {
    /// Creates a collapsed selection (caret only) at `offset`.
    pub const fn collapsed_at(offset: usize) -> Self {
        Self {
            anchor: offset,
            head: offset,
        }
    }

    /// Creates a selection covering `start..end`.
    pub const fn region(start: usize, end: usize) -> Self {
        Self {
            anchor: start,
            head: end,
        }
    }

    /// Returns the smaller byte offset of the selection.
    pub fn start(&self) -> usize {
        self.anchor.min(self.head)
    }

    /// Returns the larger byte offset of the selection.
    pub fn end(&self) -> usize {
        self.anchor.max(self.head)
    }

    /// Returns the byte range covered by the selection.
    pub fn range(&self) -> std::ops::Range<usize> {
        self.start()..self.end()
    }

    /// Returns `true` if the selection contains no text.
    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// Returns the length of the selection in bytes.
    pub fn len(&self) -> usize {
        self.end() - self.start()
    }

    /// Returns the caret offset (the moving end of the selection).
    pub fn cursor(&self) -> usize {
        self.head
    }

    /// Moves the caret to `offset`, keeping the anchor in place.
    pub fn select_to(&mut self, offset: usize) {
        self.head = offset;
    }

    /// Collapses the selection at `offset`.
    pub fn collapse_to(&mut self, offset: usize) {
        self.anchor = offset;
        self.head = offset;
    }
}
