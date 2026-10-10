//! The rope-backed text buffer.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::ops::Range;

use ropey::Rope;
use unicode_segmentation::UnicodeSegmentation;

use crate::edit::{AppliedEdit, EditKind, EditSummary, Point};
use crate::position::Position;

/// The newline convention used by a buffer.
///
/// The buffer stores the text exactly as it was loaded; this value only
/// controls which sequence is inserted for new lines. See
/// `docs/adr/0004-text-buffer.md`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LineEnding {
    /// `\n`
    #[default]
    Lf,
    /// `\r\n`
    CrLf,
}

impl LineEnding {
    /// Returns the string used when inserting a newline.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
        }
    }

    /// Guesses the dominant line ending of `text`, defaulting to [`Lf`].
    ///
    /// [`Lf`]: LineEnding::Lf
    pub fn detect(text: &str) -> Self {
        let crlf = text.matches("\r\n").count();
        let lf = text.matches('\n').count().saturating_sub(crlf);
        if crlf > lf { Self::CrLf } else { Self::Lf }
    }
}

/// An editable, rope-backed sequence of UTF-8 text.
///
/// All methods that take offsets take **byte** offsets, and all ranges are
/// half-open. Out-of-bounds offsets are clamped rather than panicking, so
/// stale offsets coming from the UI degrade gracefully instead of crashing
/// the editor.
#[derive(Clone, Debug)]
pub struct TextBuffer {
    rope: Rope,
    line_ending: LineEnding,
}

impl TextBuffer {
    /// Creates a buffer from `text`, detecting its line ending style.
    pub fn new(text: impl AsRef<str>) -> Self {
        let text = text.as_ref();
        Self {
            rope: Rope::from_str(text),
            line_ending: LineEnding::detect(text),
        }
    }

    /// Returns the buffer's total length in bytes.
    pub fn len_bytes(&self) -> usize {
        self.rope.len_bytes()
    }

    /// Returns the buffer's total length in chars.
    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    /// Returns the number of lines.
    ///
    /// Following rope terminology, a trailing newline produces a final empty
    /// line: `"a\n"` has two lines.
    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    /// Returns `true` if the buffer contains no text.
    pub fn is_empty(&self) -> bool {
        self.len_bytes() == 0
    }

    /// Returns the configured line ending.
    pub fn line_ending(&self) -> LineEnding {
        self.line_ending
    }

    /// Overrides the line ending style used for inserted newlines.
    pub fn set_line_ending(&mut self, line_ending: LineEnding) {
        self.line_ending = line_ending;
    }

    /// Returns the entire buffer as a string.
    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    /// Returns the text inside `range`, clamped to the buffer.
    pub fn slice(&self, range: Range<usize>) -> String {
        let (start, end) = self.clamp_byte_range(range);
        self.rope
            .slice(self.rope.byte_to_char(start)..self.rope.byte_to_char(end))
            .to_string()
    }

    /// Returns the number of chars in `line`, excluding the line break.
    pub fn line_content_len_chars(&self, line: usize) -> usize {
        let line = line.min(self.line_count().saturating_sub(1));
        let slice = self.rope.line(line);
        let mut len = slice.len_chars();
        if len > 0 && slice.char(len - 1) == '\n' {
            len -= 1;
            if len > 0 && slice.char(len - 1) == '\r' {
                len -= 1;
            }
        }
        len
    }

    /// Returns the content of `line` as a string, excluding the line break.
    pub fn line_content(&self, line: usize) -> String {
        if self.line_count() == 0 {
            return String::new();
        }
        let line = line.min(self.line_count() - 1);
        let start = self.rope.line_to_char(line);
        let end = start + self.line_content_len_chars(line);
        self.rope.slice(start..end).to_string()
    }

    /// Returns the byte offset where `line` starts.
    pub fn line_start_offset(&self, line: usize) -> usize {
        if self.line_count() == 0 {
            return 0;
        }
        self.rope.line_to_byte(line.min(self.line_count() - 1))
    }

    /// Returns the byte offset just past `line`'s content, before its line
    /// break.
    pub fn line_end_offset(&self, line: usize) -> usize {
        if self.line_count() == 0 {
            return 0;
        }
        let line = line.min(self.line_count() - 1);
        let start = self.rope.line_to_char(line);
        self.rope
            .char_to_byte(start + self.line_content_len_chars(line))
    }

    /// Converts a byte offset to a [`Position`].
    ///
    /// The result is clamped to the end of the buffer. Columns count chars.
    pub fn offset_to_position(&self, byte: usize) -> Position {
        let byte = byte.min(self.len_bytes());
        let line = self.rope.byte_to_line(byte);
        let line_start_char = self.rope.line_to_char(line);
        let char_offset = self.rope.byte_to_char(byte);
        Position::new(line, char_offset - line_start_char)
    }

    /// Converts a [`Position`] to a byte offset.
    ///
    /// Lines are clamped to the last line. Columns are clamped to the line's
    /// content length, so a position past the end of a line lands before
    /// its line break.
    pub fn position_to_offset(&self, position: Position) -> usize {
        let line = position.line.min(self.line_count().saturating_sub(1));
        let column = position.column.min(self.line_content_len_chars(line));
        let char_offset = self.rope.line_to_char(line) + column;
        self.rope.char_to_byte(char_offset)
    }

    /// Applies an edit, replacing `range` with `text`.
    ///
    /// Returns an [`AppliedEdit`] describing what happened, including the
    /// incremental parse information syntax layers need.
    pub fn edit(&mut self, range: Range<usize>, text: &str) -> AppliedEdit {
        let (start, end) = self.clamp_byte_range(range);
        let start_char = self.rope.byte_to_char(start);
        let end_char = self.rope.byte_to_char(end);
        let removed = self.rope.slice(start_char..end_char).to_string();
        let summary_start = self.point_at_byte(start);
        let summary_old_end = self.point_at_byte(end);

        self.rope.remove(start_char..end_char);
        self.rope.insert(start_char, text);

        let new_end = start + text.len();
        let kind = match (removed.is_empty(), text.is_empty()) {
            (true, false) => EditKind::Insert,
            (false, true) => EditKind::Delete,
            _ => EditKind::Replace,
        };
        AppliedEdit {
            range: start..end,
            inserted: text.to_owned(),
            removed,
            kind,
            summary: EditSummary {
                start_byte: start,
                old_end_byte: end,
                new_end_byte: new_end,
                start_position: summary_start,
                old_end_position: summary_old_end,
                new_end_position: self.point_at_byte(new_end),
            },
        }
    }

    /// Returns the previous grapheme boundary before `byte`.
    ///
    /// At the start of a line this returns the end of the previous line,
    /// which is the behaviour a caret should have when pressing left at a
    /// line start.
    pub fn prev_grapheme_boundary(&self, byte: usize) -> usize {
        let byte = byte.min(self.len_bytes());
        if byte == 0 {
            return 0;
        }
        let line = self.rope.byte_to_line(byte);
        let line_start = self.rope.line_to_byte(line);
        if byte > line_start {
            let content = self.line_content(line);
            if byte - line_start > content.len() {
                return line_start + content.len();
            }
            let relative = byte - line_start;
            let mut previous = 0;
            for (index, _) in content.grapheme_indices(true) {
                if index >= relative {
                    break;
                }
                previous = index;
            }
            return line_start + previous;
        }
        if line > 0 {
            self.line_end_offset(line - 1)
        } else {
            0
        }
    }

    /// Returns the next grapheme boundary after `byte`.
    ///
    /// At the end of a line this returns the start of the next line.
    pub fn next_grapheme_boundary(&self, byte: usize) -> usize {
        let byte = byte.min(self.len_bytes());
        if byte >= self.len_bytes() {
            return self.len_bytes();
        }
        let line = self.rope.byte_to_line(byte);
        let line_start = self.rope.line_to_byte(line);
        let content = self.line_content(line);
        if byte < line_start + content.len() {
            let relative = byte - line_start;
            for (index, grapheme) in content.grapheme_indices(true) {
                let end = index + grapheme.len();
                if end > relative {
                    return line_start + end;
                }
            }
        }
        if line + 1 < self.line_count() {
            self.line_start_offset(line + 1)
        } else {
            self.len_bytes()
        }
    }

    /// Returns the previous word boundary before `byte`.
    ///
    /// Word characters are ASCII alphanumerics, `_` and any non-ASCII byte,
    /// so multi-byte characters are never split. At the start of a line this
    /// returns the end of the previous line.
    pub fn prev_word_boundary(&self, byte: usize) -> usize {
        let byte = byte.min(self.len_bytes());
        if byte == 0 {
            return 0;
        }
        let line = self.rope.byte_to_line(byte);
        let line_start = self.rope.line_to_byte(line);
        if byte > line_start {
            let content = self.line_content(line);
            let relative = (byte - line_start).min(content.len());
            let bytes = content.as_bytes();
            let mut index = relative;
            while index > 0 && !is_word_byte(bytes[index - 1]) {
                index -= 1;
            }
            while index > 0 && is_word_byte(bytes[index - 1]) {
                index -= 1;
            }
            return line_start + index;
        }
        if line > 0 {
            self.line_end_offset(line - 1)
        } else {
            0
        }
    }

    /// Returns the next word boundary after `byte`.
    ///
    /// Inside a word this returns the end of that word; otherwise it skips
    /// separators and returns the end of the following word. At the end of a
    /// line this returns the start of the next line.
    pub fn next_word_boundary(&self, byte: usize) -> usize {
        let byte = byte.min(self.len_bytes());
        if byte >= self.len_bytes() {
            return self.len_bytes();
        }
        let line = self.rope.byte_to_line(byte);
        let line_start = self.rope.line_to_byte(line);
        let content = self.line_content(line);
        if byte < line_start + content.len() {
            let bytes = content.as_bytes();
            let mut index = byte - line_start;
            while index < bytes.len() && !is_word_byte(bytes[index]) {
                index += 1;
            }
            while index < bytes.len() && is_word_byte(bytes[index]) {
                index += 1;
            }
            return line_start + index;
        }
        if line + 1 < self.line_count() {
            self.line_start_offset(line + 1)
        } else {
            self.len_bytes()
        }
    }

    /// Returns a hash of the buffer contents.
    ///
    /// The hash is computed over the rope's chunks without materializing a
    /// string, so it is cheap enough to run after every edit. It is stable
    /// for the lifetime of the process, which is all dirty tracking needs.
    pub fn content_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        for chunk in self.rope.chunks() {
            chunk.hash(&mut hasher);
        }
        hasher.finish()
    }

    /// Returns the byte range of the word at or immediately before `byte`.
    ///
    /// A click on the trailing edge of a word selects that word, matching
    /// what double-click users expect. When `byte` touches no word, an
    /// empty range at `byte` is returned. The range never crosses a line.
    pub fn word_range_at(&self, byte: usize) -> Range<usize> {
        let byte = byte.min(self.len_bytes());
        let line = self.rope.byte_to_line(byte);
        let line_start = self.rope.line_to_byte(line);
        let content = self.line_content(line);
        let bytes = content.as_bytes();
        let relative = (byte - line_start).min(content.len());
        let probe = if relative < bytes.len() && is_word_byte(bytes[relative]) {
            relative
        } else if relative > 0 && is_word_byte(bytes[relative - 1]) {
            relative - 1
        } else {
            return byte..byte;
        };
        let mut start = probe;
        while start > 0 && is_word_byte(bytes[start - 1]) {
            start -= 1;
        }
        let mut end = probe;
        while end < bytes.len() && is_word_byte(bytes[end]) {
            end += 1;
        }
        line_start + start..line_start + end
    }

    /// Guesses the indentation unit used by the document.
    ///
    /// Returns a tab if the first indented line uses tabs, otherwise that
    /// line's leading space count (clamped to 1–8), defaulting to four
    /// spaces. A settings system will make this explicit later.
    pub fn indent_unit(&self) -> String {
        for line in 0..self.line_count().min(200) {
            let content = self.line_content(line);
            if content.starts_with('\t') {
                return "\t".to_owned();
            }
            let spaces = content.bytes().take_while(|byte| *byte == b' ').count();
            if spaces > 0 {
                return " ".repeat(spaces.min(8));
            }
        }
        "    ".to_owned()
    }

    fn clamp_byte_range(&self, range: Range<usize>) -> (usize, usize) {
        let len = self.len_bytes();
        let start = range.start.min(len);
        let end = range.end.clamp(start, len);
        (start, end)
    }

    fn point_at_byte(&self, byte: usize) -> Point {
        let byte = byte.min(self.len_bytes());
        let row = self.rope.byte_to_line(byte);
        let line_start = self.rope.line_to_byte(row);
        Point::new(row, byte - line_start)
    }
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte >= 0x80
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::EditKind;

    #[test]
    fn insert_and_delete_roundtrip() {
        let mut buffer = TextBuffer::new("hello world");
        let edit = buffer.edit(5..11, ", stet");
        assert_eq!(buffer.text(), "hello, stet");
        assert_eq!(edit.kind, EditKind::Replace);
        assert_eq!(edit.removed, " world");
        assert_eq!(edit.inserted, ", stet");

        let undo_edit = buffer.edit(5..11, &edit.removed);
        assert_eq!(buffer.text(), "hello world");
        assert_eq!(undo_edit.kind, EditKind::Replace);
    }

    #[test]
    fn clamps_out_of_bounds_ranges() {
        let mut buffer = TextBuffer::new("abc");
        let edit = buffer.edit(2..99, "d");
        assert_eq!(buffer.text(), "abd");
        assert_eq!(edit.range, 2..3);

        let edit = buffer.edit(99..100, "!");
        assert_eq!(buffer.text(), "abd!");
        assert_eq!(edit.range, 3..3);
    }

    #[test]
    fn positions_count_chars_not_bytes() {
        let buffer = TextBuffer::new("héllo\n日本語");
        // "héllo" is 6 bytes but 5 chars.
        assert_eq!(buffer.offset_to_position(6), Position::new(0, 5));
        // "日本語" starts at byte 7 on line 1.
        assert_eq!(buffer.offset_to_position(7), Position::new(1, 0));
        assert_eq!(buffer.offset_to_position(10), Position::new(1, 1));
        assert_eq!(buffer.position_to_offset(Position::new(0, 2)), 3);
        assert_eq!(buffer.position_to_offset(Position::new(1, 2)), 13);
    }

    #[test]
    fn positions_are_clamped_at_line_ends() {
        let buffer = TextBuffer::new("ab\ncd");
        assert_eq!(buffer.position_to_offset(Position::new(0, 50)), 2);
        assert_eq!(buffer.position_to_offset(Position::new(50, 0)), 3);
        // Past the end of the last line clamps to the buffer end.
        assert_eq!(buffer.position_to_offset(Position::new(1, 50)), 5);
    }

    #[test]
    fn grapheme_navigation_moves_over_combining_chars() {
        // "a" followed by a combining acute accent is one grapheme.
        let buffer = TextBuffer::new("ae\u{301}b");
        assert_eq!(buffer.next_grapheme_boundary(0), 1);
        assert_eq!(buffer.next_grapheme_boundary(1), 4);
        assert_eq!(buffer.next_grapheme_boundary(4), 5);
        assert_eq!(buffer.prev_grapheme_boundary(5), 4);
        assert_eq!(buffer.prev_grapheme_boundary(4), 1);
        assert_eq!(buffer.prev_grapheme_boundary(1), 0);
    }

    #[test]
    fn grapheme_navigation_crosses_lines() {
        let buffer = TextBuffer::new("ab\ncd");
        assert_eq!(buffer.next_grapheme_boundary(2), 3);
        assert_eq!(buffer.prev_grapheme_boundary(3), 2);
        assert_eq!(buffer.next_grapheme_boundary(0), 1);
        assert_eq!(buffer.prev_grapheme_boundary(0), 0);
        assert_eq!(buffer.next_grapheme_boundary(5), 5);
        assert_eq!(buffer.prev_grapheme_boundary(5), 4);
    }

    #[test]
    fn handles_crlf_line_endings() {
        let buffer = TextBuffer::new("a\r\nb\r\n");
        assert_eq!(buffer.line_ending(), LineEnding::CrLf);
        assert_eq!(buffer.line_count(), 3);
        assert_eq!(buffer.line_content(0), "a");
        assert_eq!(buffer.line_content(1), "b");
        assert_eq!(buffer.line_end_offset(0), 1);
        assert_eq!(buffer.line_start_offset(1), 3);
        assert_eq!(buffer.next_grapheme_boundary(1), 3);
        assert_eq!(buffer.prev_grapheme_boundary(3), 1);
    }

    #[test]
    fn trailing_newline_creates_empty_line() {
        let buffer = TextBuffer::new("a\n");
        assert_eq!(buffer.line_count(), 2);
        assert_eq!(buffer.line_content(1), "");
        assert_eq!(buffer.line_start_offset(1), 2);
        assert_eq!(buffer.line_end_offset(1), 2);
    }

    #[test]
    fn empty_buffer_is_one_empty_line() {
        let buffer = TextBuffer::new("");
        assert_eq!(buffer.line_count(), 1);
        assert_eq!(buffer.line_content(0), "");
        assert_eq!(buffer.offset_to_position(0), Position::new(0, 0));
        assert_eq!(buffer.position_to_offset(Position::new(0, 3)), 0);
    }

    #[test]
    fn word_navigation_stops_at_word_edges() {
        //                 0123456789 11
        let buffer = TextBuffer::new("foo bar_baz!");
        // Moving right inside a word stops at its end; from whitespace it
        // skips to the end of the following word.
        assert_eq!(buffer.next_word_boundary(0), 3);
        assert_eq!(buffer.next_word_boundary(3), 11);
        assert_eq!(buffer.next_word_boundary(11), 12);
        assert_eq!(buffer.next_word_boundary(12), 12);
        // Moving left stops at the start of the current or previous word.
        assert_eq!(buffer.prev_word_boundary(12), 4);
        assert_eq!(buffer.prev_word_boundary(11), 4);
        assert_eq!(buffer.prev_word_boundary(10), 4);
        assert_eq!(buffer.prev_word_boundary(4), 0);
        assert_eq!(buffer.prev_word_boundary(0), 0);
    }

    #[test]
    fn word_navigation_does_not_split_multibyte_chars() {
        // "héllo wörld" - "é" and "ö" are two bytes each.
        let buffer = TextBuffer::new("héllo wörld");
        let end = buffer.len_bytes();
        let boundary = buffer.prev_word_boundary(end);
        // "wörld" starts at byte 7.
        assert_eq!(boundary, 7);
        assert!(buffer.text().is_char_boundary(boundary));
    }

    #[test]
    fn word_navigation_crosses_lines() {
        let buffer = TextBuffer::new("one\ntwo");
        // From the line break, moving right lands on the next line start.
        assert_eq!(buffer.next_word_boundary(3), 4);
        assert_eq!(buffer.next_word_boundary(4), 7);
        assert_eq!(buffer.prev_word_boundary(4), 3);
        assert_eq!(buffer.prev_word_boundary(7), 4);
    }

    #[test]
    fn word_range_selects_the_word_under_the_offset() {
        let buffer = TextBuffer::new("foo bar_baz!");
        assert_eq!(buffer.word_range_at(0), 0..3);
        assert_eq!(buffer.word_range_at(2), 0..3);
        // A click on the edge of a word still selects it.
        assert_eq!(buffer.word_range_at(3), 0..3);
        assert_eq!(buffer.word_range_at(4), 4..11);
        assert_eq!(buffer.word_range_at(11), 4..11);
        // Offsets touching no word produce an empty range.
        assert_eq!(buffer.word_range_at(12), 12..12);
        let punctuation_only = TextBuffer::new("...");
        assert_eq!(punctuation_only.word_range_at(1), 1..1);
    }

    #[test]
    fn word_range_handles_multibyte_words() {
        let buffer = TextBuffer::new("héllo wörld");
        assert_eq!(buffer.word_range_at(1), 0..6);
        assert_eq!(buffer.word_range_at(8), 7..13);
    }

    #[test]
    fn word_range_does_not_cross_lines() {
        let buffer = TextBuffer::new("one\ntwo");
        assert_eq!(buffer.word_range_at(4), 4..7);
        // At the newline, the word on the line the offset belongs to wins.
        assert_eq!(buffer.word_range_at(3), 0..3);
    }

    #[test]
    fn content_hash_tracks_content_not_identity() {
        let mut buffer = TextBuffer::new("hello");
        let original = buffer.content_hash();
        assert_eq!(buffer.content_hash(), original);
        buffer.edit(5..5, "!");
        assert_ne!(buffer.content_hash(), original);
        buffer.edit(5..6, "");
        assert_eq!(buffer.content_hash(), original);
    }

    #[test]
    fn content_hash_matches_for_equal_content() {
        let a = TextBuffer::new("same\ncontent");
        let b = TextBuffer::new("same\ncontent");
        let c = TextBuffer::new("different");
        assert_eq!(a.content_hash(), b.content_hash());
        assert_ne!(a.content_hash(), c.content_hash());
    }

    #[test]
    fn indent_unit_detects_tabs_spaces_and_default() {
        assert_eq!(TextBuffer::new("fn main() {}").indent_unit(), "    ");
        assert_eq!(TextBuffer::new("a\n\tb").indent_unit(), "\t");
        assert_eq!(TextBuffer::new("a\n  b").indent_unit(), "  ");
        assert_eq!(TextBuffer::new("a\n        b").indent_unit(), "        ");
        assert_eq!(
            TextBuffer::new("a\n            b").indent_unit(),
            "        "
        );
    }

    #[test]
    fn edit_summary_tracks_byte_points() {
        let mut buffer = TextBuffer::new("ab\ncd");
        let edit = buffer.edit(3..3, "x\ny");
        assert_eq!(buffer.text(), "ab\nx\nycd");
        assert_eq!(edit.summary.start_byte, 3);
        assert_eq!(edit.summary.old_end_byte, 3);
        assert_eq!(edit.summary.new_end_byte, 6);
        assert_eq!(edit.summary.start_position, Point::new(1, 0));
        assert_eq!(edit.summary.new_end_position, Point::new(2, 1));
    }

    #[test]
    fn detects_line_endings() {
        assert_eq!(LineEnding::detect("a\nb\n"), LineEnding::Lf);
        assert_eq!(LineEnding::detect("a\r\nb\r\n"), LineEnding::CrLf);
        assert_eq!(LineEnding::detect("a\r\nb\nc\n"), LineEnding::Lf);
        assert_eq!(LineEnding::detect(""), LineEnding::Lf);
    }
}
