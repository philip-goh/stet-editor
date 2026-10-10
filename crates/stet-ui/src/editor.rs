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
    App, Bounds, ClipboardItem, Context, Element, ElementId, Entity, FocusHandle, GlobalElementId,
    Hsla, InspectorElementId, IntoElement, KeyDownEvent, LayoutId, PaintQuad, Pixels, Point,
    ScrollStrategy, ShapedLine, SharedString, Style, TextRun, UniformListScrollHandle, Window,
    fill, point, px, relative, size,
};
use stet_core::{Document, Position, Selection};
use stet_syntax::{Highlighter, Language};

use crate::theme::Theme;

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

    /// Inserts `text`, replacing the selection.
    pub fn insert_text(&mut self, text: &str, cx: &mut Context<Self>) {
        let range = self.selection.range();
        if text.is_empty() && range.is_empty() {
            return;
        }
        let edit = self.document.edit(range, text);
        self.selection.collapse_to(edit.range.start + text.len());
        self.desired_column = None;
        self.refresh(cx);
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
        if let Some(edit) = self.document.undo() {
            self.selection.collapse_to(edit.range.start);
            self.desired_column = None;
            self.refresh(cx);
        }
    }

    /// Redoes the most recently undone edit.
    pub fn redo(&mut self, cx: &mut Context<Self>) {
        if let Some(edit) = self.document.redo() {
            self.selection.collapse_to(edit.range.start);
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
        self.insert_text(&text, cx);
    }

    /// Starts a mouse selection at `byte`.
    pub fn begin_selection(&mut self, byte: usize, extend: bool, cx: &mut Context<Self>) {
        if extend {
            self.selection.select_to(byte);
        } else {
            self.selection.collapse_to(byte);
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

    /// Handles a key-down event by inserting printable characters.
    ///
    /// Named keys are routed as actions; this only deals with text input.
    /// Full IME support (composition, dead keys, CJK input) is a roadmap
    /// item and will replace this with a proper input handler.
    pub fn handle_key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let modifiers = event.keystroke.modifiers;
        if modifiers.control || modifiers.platform || modifiers.alt {
            return;
        }
        let Some(text) = event.keystroke.key_char.as_deref() else {
            return;
        };
        if text.is_empty() || text.chars().any(char::is_control) {
            return;
        }
        self.insert_text(text, cx);
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

        let mut runs: Vec<TextRun> = Vec::new();
        let mut cursor = 0;
        for span in spans {
            if span.start > cursor {
                runs.push(run(font.clone(), base_color, span.start - cursor));
            }
            runs.push(run(
                font.clone(),
                editor.theme().syntax.color(span.kind),
                span.end - span.start,
            ));
            cursor = span.end;
        }
        if cursor < content_len {
            runs.push(run(font.clone(), base_color, content_len - cursor));
        }
        if runs.is_empty() && content_len > 0 {
            runs.push(run(font, base_color, content_len));
        }

        let shaped =
            window
                .text_system()
                .shape_line(SharedString::from(content), font_size, &runs, None);

        let line_start = editor.line_start_offset(self.line);
        let line_end = line_start + content_len;
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

fn run(font: gpui::Font, color: Hsla, len: usize) -> TextRun {
    TextRun {
        len,
        font,
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    }
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
