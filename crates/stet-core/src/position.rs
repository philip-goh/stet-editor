//! Editor positions and their relation to byte offsets.

/// A zero-based line/column position in a document.
///
/// `column` counts Unicode scalar values (chars), not bytes and not
/// graphemes. It is meant for display and conversion; all editing logic in
/// this crate works on byte offsets. Use [`TextBuffer::offset_to_position`]
/// and [`TextBuffer::position_to_offset`] to convert.
///
/// [`TextBuffer::offset_to_position`]: crate::TextBuffer::offset_to_position
/// [`TextBuffer::position_to_offset`]: crate::TextBuffer::position_to_offset
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position {
    /// Zero-based line index.
    pub line: usize,
    /// Zero-based column index, measured in chars.
    pub column: usize,
}

impl Position {
    /// Creates a new position.
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

impl std::fmt::Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line + 1, self.column + 1)
    }
}
