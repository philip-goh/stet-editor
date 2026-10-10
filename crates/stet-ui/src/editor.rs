//! The editor surface: a virtualized line list with a custom text element.
//!
//! Architecture (see `docs/architecture.md`):
//!
//! - [`EditorView`] is the per-tab state holder. It owns the
//!   [`Document`](stet_core::Document), the selection, the syntax
//!   [`Highlighter`] and the scroll position. It is UI-agnostic in
//!   behaviour but GPUI entities are the unit of reactivity, so it lives
//!   here.
//! - [`LineElement`] is a GPUI `Element` that shapes and paints a single
//!   visible line with syntax runs, selection quads and the caret. It
//!   records its shaped line and bounds back into the `EditorView` so mouse
//!   hit-testing can map pixels to byte offsets.

use std::collections::HashMap;
use std::ops::Range;

use gpui::{
    App, Bounds, ClipboardItem, Context, Element, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, FocusHandle, GlobalElementId, Hsla, InspectorElementId, IntoElement,
    LayoutId, PaintQuad, Pixels, Point, ScrollStrategy, ShapedLine, SharedString, Style, TextRun,
    UTF16Selection, UnderlineStyle, UniformListScrollHandle, Window, fill, point, px, relative,
    size,
};
use stet_core::{Document, Position, Selection};
use stet_syntax::{Highlighter, Language};

use crate::theme::{SyntaxTheme, Theme};

/// Height of one editor line, in pixels.
///
/// A constant keeps the custom element, the row wrappers and the gutter in
/// agreement without relying on inherited text styles.
pub const LINE_HEIGHT: f32 = 20.0;

/// Width of the line number gutter, in pixels.
pub const GUTTER_WIDTH: f32 = 56.0;

/// Editor font size, in pixels.
pub const FONT_SIZE: f32 = 13.0;

/// A shaped, recorded line used for mouse hit-testing.
pub(crate) struct LineLayout {
    pub(crate) bounds: Bounds<Pixels>,
    shaped: ShapedLine,
}

/// Per-tab editor state.
pub struct EditorView {
    document: Document,
    selection: Selection,
    /// Remembered char column for vertical movement.
    desired_column: Option<usize>,
    highlighter: Highlighter,
    focus_handle: FocusHandle,
    scroll_handle: UniformListScrollHandle,
    /// The line range the virtualized list rendered last frame.
    visible_lines: Range<usize>,
    /// Shaped lines recorded during paint, for hit-testing.
    line_layouts: HashMap<usize, LineLayout>,
    /// Whether a mouse drag selection is in progress.
    dragging: bool,
    /// The byte range of in-progress IME composition (preedit) text, if any.
    marked_range: Option<Range<usize>>,
    theme: Theme,
}

impl EditorView {
    /// Creates an editor for `document`.
    pub fn new(document: Document, theme: Theme, cx: &mut Context<Self>) -> Self {
        let language = document
            .path()
            .map_or(Language::PlainText, Language::from_path);
        let mut highlighter = Highlighter::new(language);
        highlighter.refresh(&document.buffer().text());
        Self {
            document,
            selection: Selection::collapsed_at(0),
            desired_column: None,
            highlighter,
            focus_handle: cx.focus_handle(),
            scroll_handle: UniformListScrollHandle::default(),
            visible_lines: 0..0,
            line_layouts: HashMap::new(),
            dragging: false,
            marked_range: None,
            theme,
        }
    }

    /// Replaces the document, e.g. after reloading from disk.
    pub fn set_document(&mut self, document: Document) {
        let language = document
            .path()
            .map_or(Language::PlainText, Language::from_path);
        self.highlighter.set_language(language);
        self.document = document;
        self.selection = Selection::collapsed_at(0);
        self.desired_column = None;
        self.line_layouts.clear();
        self.marked_range = None;
        self.highlighter.refresh(&self.document.buffer().text());
    }

    /// Returns the document.
    pub fn document(&self) -> &Document {
        &self.document
    }

    /// Returns the mutable document for operations the view does not wrap.
    pub fn document_mut(&mut self) -> &mut Document {
        &mut self.document
    }

    /// Returns the focus handle for this editor.
    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus_handle
    }

    /// Returns the scroll handle shared with the virtualized line list.
    pub fn scroll_handle(&self) -> &UniformListScrollHandle {
        &self.scroll_handle
    }

    /// Returns the theme colors the editor paints with.
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Returns the language used for highlighting.
    pub fn language(&self) -> Language {
        self.highlighter.language()
    }

    /// Returns the current selection.
    pub fn selection(&self) -> Selection {
        self.selection
    }

    /// Returns `true` while a drag selection is in progress.
    pub fn dragging(&self) -> bool {
        self.dragging
    }

    /// Returns the number of lines in the document.
    pub fn line_count(&self) -> usize {
        self.document.buffer().line_count()
    }

    /// Returns the content of `line`, excluding its line break.
    pub fn line_content(&self, line: usize) -> String {
        self.document.buffer().line_content(line)
    }

    /// Returns the byte offset where `line` starts.
    pub fn line_start_offset(&self, line: usize) -> usize {
        self.document.buffer().line_start_offset(line)
    }

    /// Returns the highlighted, line-relative spans for `line`.
    pub fn line_spans(&self, line: usize) -> &[stet_syntax::HighlightSpan] {
        self.highlighter.line_spans(line)
    }

    /// Returns the line containing the caret.
    pub fn cursor_line(&self) -> usize {
        self.document
            .buffer()
            .offset_to_position(self.selection.cursor())
            .line
    }

    /// Returns the cursor position as one-based `(line, column)` for the
    /// status bar.
    pub fn cursor_status(&self) -> (usize, usize) {
        let position = self
            .document
            .buffer()
            .offset_to_position(self.selection.cursor());
        (position.line + 1, position.column + 1)
    }

    /// Records the line range rendered by the virtualized list.
    pub fn set_visible_lines(&mut self, range: Range<usize>) {
        self.visible_lines = range;
        // Keep the hit-test map bounded; off-screen lines are dropped.
        if self.line_layouts.len() > 4 * (self.visible_lines.len().max(64)) {
            let visible = self.visible_lines.clone();
            self.line_layouts.retain(|line, _| visible.contains(line));
        }
    }

    /// Records a painted line for mouse hit-testing.
    pub(crate) fn record_line_layout(&mut self, line: usize, layout: LineLayout) {
        self.line_layouts.insert(line, layout);
    }

    /// Maps a window point to a `(line, byte offset)` pair, if it hits a
    /// line that was painted.
    ///
    /// The returned offset is always on a char boundary, even if the shaped
    /// line reports something in between.
    pub fn hit_test(&self, position: Point<Pixels>) -> Option<(usize, usize)> {
        for (line, layout) in &self.line_layouts {
            if position.y >= layout.bounds.top() && position.y <= layout.bounds.bottom() {
                let x = position.x - layout.bounds.left();
                let content = self.line_content(*line);
                let mut index = layout.shaped.closest_index_for_x(x).min(content.len());
                while index > 0 && !content.is_char_boundary(index) {
                    index -= 1;
                }
                return Some((*line, self.line_start_offset(*line) + index));
            }
        }
        None
    }

    /// Inserts user-typed `text`, replacing the selection.
    ///
    /// Typed text gets editor affordances: closing brackets and quote pairs
    /// are inserted around a collapsed caret, wrap an active selection, and
    /// typing a closing character next to its match skips over it.
    /// Programmatic insertion (pasting, IME commits) uses
    /// [`EditorView::insert_plain`] instead.
    pub fn insert_text(&mut self, text: &str, cx: &mut Context<Self>) {
        if self.selection.is_empty() {
            // Typing the character that is already ahead of the caret moves
            // over it instead of doubling it.
            if is_skippable(text) {
                let cursor = self.selection.cursor();
                let buffer = self.document.buffer();
                let next = buffer.next_grapheme_boundary(cursor);
                if buffer.slice(cursor..next) == text {
                    self.selection.collapse_to(next);
                    cx.notify();
                    return;
                }
            }
            if let Some(close) = matching_close_for(text) {
                // Don't auto-close quotes in the middle of a word
                // ("don't"). Brackets always pair.
                if !(is_quote(text) && self.previous_char_is_word()) {
                    let at = self.selection.cursor();
                    let edit = self.document.edit(at..at, &format!("{text}{close}"));
                    self.selection.collapse_to(edit.range.start + text.len());
                    self.desired_column = None;
                    self.refresh(cx);
                    return;
                }
            }
        } else if let Some(close) = matching_close_for(text) {
            let range = self.selection.range();
            let selected = self.document.buffer().slice(range.clone());
            let edit = self
                .document
                .edit(range, &format!("{text}{selected}{close}"));
            let start = edit.range.start + text.len();
            self.selection = Selection::region(start, start + selected.len());
            self.desired_column = None;
            self.refresh(cx);
            return;
        }

        self.insert_plain(text, cx);
    }

    /// Inserts `text`, replacing the selection, with no automatic pairs.
    pub fn insert_plain(&mut self, text: &str, cx: &mut Context<Self>) {
        let range = self.selection.range();
        if text.is_empty() && range.is_empty() {
            return;
        }
        let edit = self.document.edit(range, text);
        self.selection.collapse_to(edit.range.start + text.len());
        self.desired_column = None;
        self.refresh(cx);
    }

    /// Returns `true` if the char immediately before the caret is a word
    /// character.
    fn previous_char_is_word(&self) -> bool {
        let cursor = self.selection.cursor();
        if cursor == 0 {
            return false;
        }
        let buffer = self.document.buffer();
        let previous = buffer.prev_grapheme_boundary(cursor);
        buffer
            .slice(previous..cursor)
            .chars()
            .next_back()
            .is_some_and(|character| character.is_alphanumeric() || character == '_')
    }

    /// Inserts a line break, copying the current line's leading whitespace.
    pub fn insert_newline(&mut self, cx: &mut Context<Self>) {
        let (newline, indent) = {
            let buffer = self.document.buffer();
            let position = buffer.offset_to_position(self.selection.cursor());
            let line_content = buffer.line_content(position.line);
            let indent: String = line_content
                .chars()
                .take_while(|c| *c == ' ' || *c == '\t')
                .collect();
            (buffer.line_ending().as_str(), indent)
        };
        self.insert_text(&format!("{newline}{indent}"), cx);
    }

    /// Deletes the previous grapheme, or the selection.
    pub fn delete_backward(&mut self, cx: &mut Context<Self>) {
        let range = if self.selection.is_empty() {
            let previous = self
                .document
                .buffer()
                .prev_grapheme_boundary(self.selection.cursor());
            previous..self.selection.cursor()
        } else {
            self.selection.range()
        };
        if range.is_empty() {
            return;
        }
        let edit = self.document.edit(range, "");
        self.selection.collapse_to(edit.range.start);
        self.desired_column = None;
        self.refresh(cx);
    }

    /// Deletes the next grapheme, or the selection.
    pub fn delete_forward(&mut self, cx: &mut Context<Self>) {
        let range = if self.selection.is_empty() {
            let next = self
                .document
                .buffer()
                .next_grapheme_boundary(self.selection.cursor());
            self.selection.cursor()..next
        } else {
            self.selection.range()
        };
        if range.is_empty() {
            return;
        }
        let edit = self.document.edit(range, "");
        self.selection.collapse_to(edit.range.start);
        self.desired_column = None;
        self.refresh(cx);
    }

    /// Moves the caret one grapheme horizontally.
    pub fn move_horizontal(&mut self, right: bool, extend: bool, cx: &mut Context<Self>) {
        let cursor = self.selection.cursor();
        let buffer = self.document.buffer();
        let target = if right {
            buffer.next_grapheme_boundary(cursor)
        } else {
            buffer.prev_grapheme_boundary(cursor)
        };
        self.desired_column = None;
        self.move_to(target, extend, cx);
    }

    /// Moves the caret one line vertically, restoring the desired column.
    pub fn move_vertical(&mut self, down: bool, extend: bool, cx: &mut Context<Self>) {
        let (last_line, desired, target) = {
            let buffer = self.document.buffer();
            let current = buffer.offset_to_position(self.selection.cursor());
            let desired = self.desired_column.unwrap_or(current.column);
            let last_line = buffer.line_count().saturating_sub(1);
            let target_line = if down {
                (current.line + 1).min(last_line)
            } else {
                current.line.saturating_sub(1)
            };
            (
                last_line,
                desired,
                buffer.position_to_offset(Position::new(target_line, desired)),
            )
        };
        if last_line == 0 {
            return;
        }
        self.desired_column = Some(desired);
        self.move_to(target, extend, cx);
    }

    /// Moves the caret one word horizontally.
    pub fn move_word(&mut self, right: bool, extend: bool, cx: &mut Context<Self>) {
        let cursor = self.selection.cursor();
        let buffer = self.document.buffer();
        let target = if right {
            buffer.next_word_boundary(cursor)
        } else {
            buffer.prev_word_boundary(cursor)
        };
        self.desired_column = None;
        self.move_to(target, extend, cx);
    }

    /// Moves the caret to the start of its line.
    pub fn move_line_start(&mut self, extend: bool, cx: &mut Context<Self>) {
        let line = self.cursor_line();
        // Home toggles between the first non-whitespace char and column 0.
        let line_start = self.line_start_offset(line);
        let content = self.line_content(line);
        let first_non_ws = content
            .char_indices()
            .find(|(_, c)| !c.is_whitespace())
            .map_or(0, |(index, _)| index);
        let target = if self.selection.cursor() == line_start + first_non_ws {
            line_start
        } else {
            line_start + first_non_ws
        };
        self.desired_column = None;
        self.move_to(target, extend, cx);
    }

    /// Moves the caret to the end of its line.
    pub fn move_line_end(&mut self, extend: bool, cx: &mut Context<Self>) {
        let line = self.cursor_line();
        let buffer = self.document.buffer();
        let target = buffer.line_end_offset(line);
        self.desired_column = None;
        self.move_to(target, extend, cx);
    }

    /// Moves the caret to the start of the document.
    pub fn move_document_start(&mut self, extend: bool, cx: &mut Context<Self>) {
        self.desired_column = None;
        self.move_to(0, extend, cx);
    }

    /// Moves the caret to the end of the document.
    pub fn move_document_end(&mut self, extend: bool, cx: &mut Context<Self>) {
        self.desired_column = None;
        let end = self.document.buffer().len_bytes();
        self.move_to(end, extend, cx);
    }

    /// Selects the entire document.
    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        let end = self.document.buffer().len_bytes();
        self.selection = Selection::region(0, end);
        cx.notify();
    }

    /// Collapses the selection to the caret, if it is not already empty.
    pub fn clear_selection(&mut self, cx: &mut Context<Self>) {
        if !self.selection.is_empty() {
            self.selection.collapse_to(self.selection.cursor());
            cx.notify();
        }
    }

    /// Undoes the most recent edit.
    pub fn undo(&mut self, cx: &mut Context<Self>) {
        if let Some(step) = self.document.undo() {
            self.selection.collapse_to(step.start());
            self.desired_column = None;
            self.refresh(cx);
        }
    }

    /// Redoes the most recently undone edit.
    pub fn redo(&mut self, cx: &mut Context<Self>) {
        if let Some(step) = self.document.redo() {
            self.selection.collapse_to(step.start());
            self.desired_column = None;
            self.refresh(cx);
        }
    }

    /// Copies the selection to the system clipboard.
    pub fn copy(&mut self, cx: &mut Context<Self>) {
        if self.selection.is_empty() {
            return;
        }
        let text = self.document.buffer().slice(self.selection.range());
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }

    /// Cuts the selection to the system clipboard.
    pub fn cut(&mut self, cx: &mut Context<Self>) {
        if self.selection.is_empty() {
            return;
        }
        self.copy(cx);
        let edit = self.document.edit(self.selection.range(), "");
        self.selection.collapse_to(edit.range.start);
        self.refresh(cx);
    }

    /// Pastes the clipboard over the selection.
    pub fn paste(&mut self, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        self.insert_plain(&text, cx);
    }

    /// Indents or outdents the selected lines.
    ///
    /// A collapsed selection indents the current line. A multi-line
    /// selection block-indents every line it touches; the whole batch is a
    /// single undo step.
    pub fn indent(&mut self, outdent: bool, cx: &mut Context<Self>) {
        let unit = self.document.buffer().indent_unit();

        if self.selection.is_empty() {
            let cursor = self.selection.cursor();
            let line = self.document.buffer().offset_to_position(cursor).line;
            let line_start = self.document.buffer().line_start_offset(line);
            if outdent {
                let content = self.document.buffer().line_content(line);
                let prefix = leading_indent_len(&content, &unit);
                if prefix == 0 {
                    return;
                }
                let edit = self.document.edit(line_start..line_start + prefix, "");
                let new_cursor = cursor.saturating_sub(prefix).max(edit.range.start);
                self.selection.collapse_to(new_cursor);
            } else {
                let edit = self.document.edit(line_start..line_start, &unit);
                self.selection.collapse_to(edit.range.start + unit.len());
            }
            self.desired_column = None;
            self.refresh(cx);
            return;
        }

        let (first_line, last_line) = {
            let buffer = self.document.buffer();
            let first = buffer.offset_to_position(self.selection.start()).line;
            let mut last = buffer.offset_to_position(self.selection.end()).line;
            // A selection ending exactly at a line start does not include
            // that line.
            if self.selection.end() == buffer.line_start_offset(last) && last > first {
                last -= 1;
            }
            (first, last)
        };

        let mut edits: Vec<(std::ops::Range<usize>, String)> = Vec::new();
        for line in first_line..=last_line {
            let buffer = self.document.buffer();
            let line_start = buffer.line_start_offset(line);
            if outdent {
                let content = buffer.line_content(line);
                let prefix = leading_indent_len(&content, &unit);
                if prefix > 0 {
                    edits.push((line_start..line_start + prefix, String::new()));
                }
            } else {
                edits.push((line_start..line_start, unit.clone()));
            }
        }
        if edits.is_empty() {
            return;
        }

        self.document.edit_batch(edits);
        let buffer = self.document.buffer();
        self.selection = Selection::region(
            buffer.line_start_offset(first_line),
            buffer.line_end_offset(last_line),
        );
        self.desired_column = None;
        self.refresh(cx);
    }

    /// Starts a mouse selection at `byte`.
    ///
    /// `click_count` selects the unit: one click places the caret, two
    /// clicks select the word, three or more select the line. Shift-click
    /// extends the existing selection.
    pub fn begin_selection(
        &mut self,
        byte: usize,
        extend: bool,
        click_count: usize,
        cx: &mut Context<Self>,
    ) {
        match click_count {
            2 => {
                let range = self.document.buffer().word_range_at(byte);
                self.selection = Selection::region(range.start, range.end);
            }
            count if count >= 3 => {
                let buffer = self.document.buffer();
                let line = buffer.offset_to_position(byte).line;
                self.selection =
                    Selection::region(buffer.line_start_offset(line), buffer.line_end_offset(line));
            }
            _ => {
                if extend {
                    self.selection.select_to(byte);
                } else {
                    self.selection.collapse_to(byte);
                }
            }
        }
        self.desired_column = None;
        self.dragging = true;
        self.ensure_cursor_visible();
        cx.notify();
    }

    /// Extends an in-progress mouse selection to `byte`.
    pub fn extend_selection(&mut self, byte: usize, cx: &mut Context<Self>) {
        self.selection.select_to(byte);
        self.ensure_cursor_visible();
        cx.notify();
    }

    /// Ends a mouse drag.
    pub fn end_drag(&mut self, cx: &mut Context<Self>) {
        if self.dragging {
            self.dragging = false;
            cx.notify();
        }
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        let buffer = self.document.buffer();
        buffer.byte_to_utf16(range.start)..buffer.byte_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        let buffer = self.document.buffer();
        buffer.utf16_to_byte(range.start)..buffer.utf16_to_byte(range.end)
    }

    fn move_to(&mut self, offset: usize, extend: bool, cx: &mut Context<Self>) {
        if extend {
            self.selection.select_to(offset);
        } else {
            self.selection.collapse_to(offset);
        }
        self.ensure_cursor_visible();
        cx.notify();
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.highlighter.refresh(&self.document.buffer().text());
        self.ensure_cursor_visible();
        cx.notify();
    }

    fn ensure_cursor_visible(&mut self) {
        if self.visible_lines.is_empty() {
            return;
        }
        let line = self.cursor_line();
        if line < self.visible_lines.start {
            self.scroll_handle.scroll_to_item(line, ScrollStrategy::Top);
        } else if line >= self.visible_lines.end {
            self.scroll_handle
                .scroll_to_item(line, ScrollStrategy::Bottom);
        }
    }
}

/// Platform text input: plain typing, paste-through-IME and composition.
///
/// GPUI routes all printable text through the focused element's
/// [`EntityInputHandler`]; named keys (arrows, shortcuts) arrive as actions
/// instead. All offsets at this boundary are UTF-16 code units, which is
/// what the platforms speak.
impl EntityInputHandler for EditorView {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.document.buffer().slice(range))
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selection.range()),
            reversed: self.selection.head < self.selection.anchor,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Plain typing (no explicit range, no composition) goes through the
        // editor affordances in `insert_text`.
        if range_utf16.is_none() && self.marked_range.is_none() {
            self.insert_text(new_text, cx);
            return;
        }

        let range = range_utf16
            .map(|range| self.range_from_utf16(&range))
            .or_else(|| self.marked_range.clone())
            .unwrap_or_else(|| self.selection.range());
        let edit = self.document.edit(range, new_text);
        self.selection
            .collapse_to(edit.range.start + new_text.len());
        self.marked_range = None;
        self.desired_column = None;
        self.refresh(cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .map(|range| self.range_from_utf16(&range))
            .or_else(|| self.marked_range.clone())
            .unwrap_or_else(|| self.selection.range());
        let edit = self.document.edit(range, new_text);
        let start = edit.range.start;
        let end = start + new_text.len();
        self.marked_range = (!new_text.is_empty()).then_some(start..end);
        self.selection = match new_selected_range_utf16 {
            Some(selected) => Selection::region(
                start + utf16_to_byte_in(new_text, selected.start),
                start + utf16_to_byte_in(new_text, selected.end),
            ),
            None => Selection::region(end, end),
        };
        self.desired_column = None;
        self.refresh(cx);
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let buffer = self.document.buffer();
        let start = buffer.utf16_to_byte(range_utf16.start);
        let end = buffer.utf16_to_byte(range_utf16.end);
        let position = buffer.offset_to_position(start);

        let Some(layout) = self.line_layouts.get(&position.line) else {
            // The line is not painted; the platform only needs a plausible
            // anchor for the IME candidate window.
            return Some(Bounds::new(bounds.origin, size(px(1.), px(LINE_HEIGHT))));
        };

        let line_start = buffer.line_start_offset(position.line);
        let content_len = buffer.line_content(position.line).len();
        let x0 = layout
            .shaped
            .x_for_index((start - line_start).min(content_len));
        let x1 = layout
            .shaped
            .x_for_index((end - line_start).min(content_len));
        Some(Bounds::from_corners(
            point(layout.bounds.left() + x0, layout.bounds.top()),
            point(
                layout.bounds.left() + x1.max(x0 + px(1.)),
                layout.bounds.bottom(),
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let (_line, byte) = self.hit_test(point)?;
        Some(self.document.buffer().byte_to_utf16(byte))
    }
}

/// The input area element.
///
/// It has no visuals; painting it registers the editor's
/// [`EntityInputHandler`] with the window for the frame. GPUI only registers
/// it while the editor's focus handle is focused, so other focused surfaces
/// (like the command palette) are unaffected.
pub(crate) struct InputAreaElement {
    pub(crate) editor: Entity<EditorView>,
}

impl IntoElement for InputAreaElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for InputAreaElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.editor.read(cx).focus_handle().clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.editor.clone()),
            cx,
        );
    }
}

/// The prepaint output of a [`LineElement`].
pub(crate) struct LinePrepaint {
    shaped: Option<ShapedLine>,
    selection: Option<PaintQuad>,
    caret: Option<PaintQuad>,
}

/// A custom element that renders one line of the document.
pub(crate) struct LineElement {
    /// The editor this line belongs to.
    pub(crate) editor: Entity<EditorView>,
    /// The zero-based line index.
    pub(crate) line: usize,
}

impl IntoElement for LineElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for LineElement {
    type RequestLayoutState = ();
    type PrepaintState = LinePrepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = px(LINE_HEIGHT).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let editor = self.editor.read(cx);
        let content = editor.line_content(self.line);
        let content_len = content.len();
        let spans = editor.line_spans(self.line);

        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let font = style.font();
        let base_color = style.color;

        let line_start = editor.line_start_offset(self.line);
        let line_end = line_start + content_len;
        let marked = editor.marked_range.as_ref().and_then(|range| {
            (range.start < line_end && range.end > line_start).then(|| {
                (range.start.max(line_start) - line_start)..(range.end.min(line_end) - line_start)
            })
        });
        let runs = line_runs(
            content_len,
            spans,
            &editor.theme().syntax,
            base_color,
            font,
            marked,
            editor.theme().foreground,
        );

        let shaped =
            window
                .text_system()
                .shape_line(SharedString::from(content), font_size, &runs, None);

        let selection = editor.selection();
        let selection_quad = selection_quad(
            &shaped,
            bounds,
            editor.theme(),
            selection,
            line_start,
            line_end,
            content_len,
        );
        let caret_quad = caret_quad(
            &shaped,
            bounds,
            editor.theme(),
            editor.focus_handle().is_focused(window),
            selection,
            line_start,
            line_end,
        );

        LinePrepaint {
            shaped: Some(shaped),
            selection: selection_quad,
            caret: caret_quad,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(quad) = prepaint.selection.take() {
            window.paint_quad(quad);
        }

        let shaped = prepaint.shaped.take();
        if let Some(line) = &shaped {
            let _ = line.paint(bounds.origin, px(LINE_HEIGHT), window, cx);
        }

        if let Some(quad) = prepaint.caret.take() {
            window.paint_quad(quad);
        }

        if let Some(shaped) = shaped {
            self.editor.update(cx, |editor, _cx| {
                editor.record_line_layout(self.line, LineLayout { bounds, shaped });
            });
        }
    }
}

/// Returns the byte length of the removable indent prefix of `content`.
///
/// A tab removes one tab; otherwise up to `unit`'s width in leading spaces
/// is removed (four spaces when the document indents with tabs).
fn leading_indent_len(content: &str, unit: &str) -> usize {
    if content.starts_with('\t') {
        return 1;
    }
    let spaces = content.bytes().take_while(|byte| *byte == b' ').count();
    if unit == "\t" {
        spaces.min(4)
    } else {
        spaces.min(unit.len())
    }
}

/// Returns the closing character that should be inserted for `text`, if
/// `text` is an opening bracket or a quote.
fn matching_close_for(text: &str) -> Option<&'static str> {
    match text {
        "(" => Some(")"),
        "[" => Some("]"),
        "{" => Some("}"),
        "\"" => Some("\""),
        "'" => Some("'"),
        "`" => Some("`"),
        _ => None,
    }
}

/// Returns `true` if typing `text` next to its match should skip over it.
fn is_skippable(text: &str) -> bool {
    matches!(text, ")" | "]" | "}" | "\"" | "'" | "`")
}

fn is_quote(text: &str) -> bool {
    matches!(text, "\"" | "'" | "`")
}

/// Converts a UTF-16 offset inside `text` to a byte offset, clamping.
fn utf16_to_byte_in(text: &str, utf16: usize) -> usize {
    let mut units = 0;
    let mut bytes = 0;
    for character in text.chars() {
        if units >= utf16 {
            break;
        }
        units += character.len_utf16();
        bytes += character.len_utf8();
    }
    bytes
}

/// Builds the styled runs for one line.
///
/// Syntax spans and the IME preedit range (if any) are merged by cutting
/// the line at every boundary, so a run is never split incorrectly when a
/// preedit starts or ends inside a highlighted span.
#[allow(clippy::too_many_arguments)]
fn line_runs(
    content_len: usize,
    spans: &[stet_syntax::HighlightSpan],
    syntax: &SyntaxTheme,
    base_color: Hsla,
    font: gpui::Font,
    marked: Option<Range<usize>>,
    mark_color: Hsla,
) -> Vec<TextRun> {
    let mut cuts = vec![0, content_len];
    for span in spans {
        cuts.push(span.start);
        cuts.push(span.end);
    }
    if let Some(marked) = &marked {
        cuts.push(marked.start);
        cuts.push(marked.end);
    }
    cuts.retain(|cut| *cut <= content_len);
    cuts.sort_unstable();
    cuts.dedup();

    let mut runs = Vec::new();
    let mut span_index = 0;
    for pair in cuts.windows(2) {
        let (start, end) = (pair[0], pair[1]);
        if start >= end {
            continue;
        }
        while span_index < spans.len() && spans[span_index].end <= start {
            span_index += 1;
        }
        let color = spans
            .get(span_index)
            .filter(|span| span.start <= start && start < span.end)
            .map_or(base_color, |span| syntax.color(span.kind));
        let underline = marked
            .as_ref()
            .is_some_and(|marked| marked.start <= start && start < marked.end)
            .then_some(UnderlineStyle {
                color: Some(mark_color),
                thickness: px(1.),
                wavy: false,
            });
        runs.push(TextRun {
            len: end - start,
            font: font.clone(),
            color,
            background_color: None,
            underline,
            strikethrough: None,
        });
    }
    runs
}

fn selection_quad(
    shaped: &ShapedLine,
    bounds: Bounds<Pixels>,
    theme: &Theme,
    selection: Selection,
    line_start: usize,
    line_end: usize,
    content_len: usize,
) -> Option<PaintQuad> {
    if selection.is_empty() || selection.start() > line_end || selection.end() < line_start {
        return None;
    }
    let start = selection.start().max(line_start) - line_start;
    let end = if selection.end() >= line_end {
        content_len
    } else {
        selection.end() - line_start
    };
    if start >= end {
        return None;
    }
    let x0 = shaped.x_for_index(start);
    let x1 = shaped.x_for_index(end);
    Some(fill(
        Bounds::new(
            point(bounds.left() + x0, bounds.top()),
            size(x1 - x0, bounds.size.height),
        ),
        theme.selection,
    ))
}

fn caret_quad(
    shaped: &ShapedLine,
    bounds: Bounds<Pixels>,
    theme: &Theme,
    focused: bool,
    selection: Selection,
    line_start: usize,
    line_end: usize,
) -> Option<PaintQuad> {
    if !focused {
        return None;
    }
    let caret = selection.cursor();
    if caret < line_start || caret > line_end {
        return None;
    }
    let x = shaped.x_for_index(caret - line_start);
    Some(fill(
        Bounds::new(
            point(bounds.left() + x, bounds.top()),
            size(px(2.), bounds.size.height),
        ),
        theme.caret,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{TestAppContext, VisualTestContext, div, prelude::*};

    struct TestRoot;

    impl gpui::Render for TestRoot {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    fn setup(cx: &mut TestAppContext) -> (Entity<EditorView>, &mut VisualTestContext) {
        let (_root, cx) = cx.add_window_view(|_window, _cx| TestRoot);
        let editor = cx.new(|cx| EditorView::new(Document::untitled(), Theme::dark(), cx));
        (editor, cx)
    }

    fn text_of(editor: &Entity<EditorView>, cx: &mut VisualTestContext) -> String {
        editor.update(cx, |editor, _cx| editor.document().buffer().text())
    }

    #[gpui::test]
    fn typing_inserts_text(cx: &mut TestAppContext) {
        let (editor, cx) = setup(cx);
        editor.update_in(cx, |editor, window, cx| {
            editor.replace_text_in_range(None, "hello", window, cx);
            editor.replace_text_in_range(None, " world", window, cx);
        });
        assert_eq!(text_of(&editor, cx), "hello world");
        assert_eq!(
            editor.update(cx, |editor, _cx| editor.cursor_status()),
            (1, 12)
        );
    }

    #[gpui::test]
    fn brackets_pair_and_skip(cx: &mut TestAppContext) {
        let (editor, cx) = setup(cx);
        editor.update_in(cx, |editor, window, cx| {
            editor.replace_text_in_range(None, "(", window, cx);
        });
        assert_eq!(text_of(&editor, cx), "()");
        assert_eq!(
            editor.update(cx, |editor, _cx| editor.selection().cursor()),
            1
        );

        editor.update_in(cx, |editor, window, cx| {
            editor.replace_text_in_range(None, ")", window, cx);
        });
        assert_eq!(text_of(&editor, cx), "()");
        assert_eq!(
            editor.update(cx, |editor, _cx| editor.selection().cursor()),
            2
        );
    }

    #[gpui::test]
    fn typing_a_quote_wraps_the_selection(cx: &mut TestAppContext) {
        let (editor, cx) = setup(cx);
        editor.update_in(cx, |editor, window, cx| {
            editor.replace_text_in_range(None, "word", window, cx);
            editor.select_all(cx);
            editor.replace_text_in_range(None, "\"", window, cx);
        });
        assert_eq!(text_of(&editor, cx), "\"word\"");
    }

    #[gpui::test]
    fn apostrophes_are_not_doubled_inside_words(cx: &mut TestAppContext) {
        let (editor, cx) = setup(cx);
        editor.update_in(cx, |editor, window, cx| {
            editor.replace_text_in_range(None, "don", window, cx);
            editor.replace_text_in_range(None, "'", window, cx);
            editor.replace_text_in_range(None, "t", window, cx);
        });
        assert_eq!(text_of(&editor, cx), "don't");
    }

    #[gpui::test]
    fn composition_marks_and_commits_text(cx: &mut TestAppContext) {
        let (editor, cx) = setup(cx);
        editor.update_in(cx, |editor, window, cx| {
            editor.replace_and_mark_text_in_range(None, "に", Some(0..1), window, cx);
        });
        assert_eq!(text_of(&editor, cx), "に");
        let marked = editor.update_in(cx, |editor, window, cx| {
            editor.marked_text_range(window, cx)
        });
        assert_eq!(marked, Some(0..1));

        editor.update_in(cx, |editor, window, cx| {
            editor.replace_text_in_range(None, "日本", window, cx);
        });
        assert_eq!(text_of(&editor, cx), "日本");
        let marked = editor.update_in(cx, |editor, window, cx| {
            editor.marked_text_range(window, cx)
        });
        assert_eq!(marked, None);
    }

    #[gpui::test]
    fn ranges_are_reported_in_utf16(cx: &mut TestAppContext) {
        let (editor, cx) = setup(cx);
        editor.update_in(cx, |editor, window, cx| {
            editor.replace_text_in_range(None, "a😀b", window, cx);
            editor.select_all(cx);
        });
        let selection = editor.update_in(cx, |editor, window, cx| {
            editor.selected_text_range(false, window, cx)
        });
        assert_eq!(selection.map(|selection| selection.range), Some(0..4));

        let text = editor.update_in(cx, |editor, window, cx| {
            editor.text_for_range(1..3, &mut None, window, cx)
        });
        assert_eq!(text.as_deref(), Some("😀"));
    }
}
