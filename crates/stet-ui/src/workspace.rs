//! The application shell: tabs, toolbar, status bar, command palette and
//! action routing.
//!
//! All commands flow through the `on_*` action handlers so that keyboard
//! shortcuts, toolbar buttons and palette entries share one implementation.
//! The palette, when open, intercepts the editing actions and routes them to
//! its own state instead of the editor.

use std::path::PathBuf;

use gpui::prelude::*;
use gpui::{
    ClickEvent, Context, ElementId, Entity, FocusHandle, KeyDownEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PathPromptOptions, PromptLevel, SharedString, Window, div, px,
    rgba, uniform_list,
};
use stet_core::Document;

use crate::actions::*;
use crate::editor::{EditorView, GUTTER_WIDTH, InputAreaElement, LINE_HEIGHT, LineElement};
use crate::palette::{Palette, PaletteCommand};
use crate::theme::Theme;

/// The monospace font used by the editor, chosen per platform.
const EDITOR_FONT: &str = if cfg!(target_os = "macos") {
    "Menlo"
} else if cfg!(target_os = "windows") {
    "Consolas"
} else {
    "DejaVu Sans Mono"
};

/// The root view.
pub struct Workspace {
    tabs: Vec<Entity<EditorView>>,
    active: usize,
    palette: Palette,
    theme: Theme,
    status_message: Option<SharedString>,
}

impl Workspace {
    /// Creates a workspace, opening `initial_file` when given.
    pub fn new(initial_file: Option<PathBuf>, cx: &mut Context<Self>) -> Self {
        let theme = Theme::dark();
        let mut status_message = None;
        let document = match initial_file {
            Some(path) => match Document::open(&path) {
                Ok(document) => document,
                Err(error) => {
                    status_message = Some(SharedString::from(error.to_string()));
                    Document::untitled()
                }
            },
            None => Document::untitled(),
        };

        let editor = cx.new(|cx| EditorView::new(document, theme.clone(), cx));
        cx.observe(&editor, |_, _, cx| cx.notify()).detach();

        Self {
            tabs: vec![editor],
            active: 0,
            palette: Palette::new(cx.focus_handle()),
            theme,
            status_message,
        }
    }

    /// Returns the active editor.
    pub fn active_editor(&self) -> &Entity<EditorView> {
        &self.tabs[self.active]
    }

    // ---------------------------------------------------------------
    // Editor commands (shared by actions, palette and toolbar)
    // ---------------------------------------------------------------

    /// Creates a new untitled tab and focuses it.
    pub fn perform_new_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let editor = self.new_editor(Document::untitled(), cx);
        self.tabs.push(editor);
        self.active = self.tabs.len() - 1;
        self.focus_active_editor(window, cx);
        cx.notify();
    }

    /// Shows the open dialog and opens the chosen file.
    pub fn perform_open_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Open File".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await {
                for path in paths {
                    let _ = this.update_in(cx, |workspace, window, cx| {
                        workspace.open_path(path, window, cx);
                    });
                }
            }
        })
        .detach();
    }

    /// Opens `path` in a new tab, reusing a pristine scratch tab if there is
    /// one.
    pub fn open_path(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        match Document::open(&path) {
            Ok(document) => {
                let active = self.active_editor().clone();
                let is_scratch = {
                    let editor = active.read(cx);
                    let document = editor.document();
                    document.path().is_none()
                        && !document.is_dirty()
                        && document.buffer().is_empty()
                };
                if is_scratch {
                    active.update(cx, |editor, _cx| editor.set_document(document));
                    self.focus_active_editor(window, cx);
                } else {
                    let editor = self.new_editor(document, cx);
                    self.tabs.push(editor);
                    self.active = self.tabs.len() - 1;
                    self.focus_active_editor(window, cx);
                }
                cx.notify();
            }
            Err(error) => self.set_status(error.to_string(), cx),
        }
    }

    /// Saves the active document to its existing path.
    pub fn perform_save(&mut self, cx: &mut Context<Self>) {
        let editor = self.active_editor().clone();
        let result = editor.update(cx, |editor, _cx| editor.document_mut().save());
        match result {
            Ok(()) => {
                let name = editor.read(cx).document().display_name();
                self.set_status(format!("Saved {name}"), cx);
            }
            Err(error) => self.set_status(error.to_string(), cx),
        }
    }

    /// Shows the save dialog and saves the active document to the chosen
    /// path.
    pub fn perform_save_as(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let suggested = self.active_editor().read(cx).document().display_name();
        let directory = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let receiver = cx.prompt_for_new_path(&directory, Some(&suggested));
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(path))) = receiver.await {
                let _ = this.update_in(cx, |workspace, _window, cx| {
                    let editor = workspace.active_editor().clone();
                    let result = editor.update(cx, |editor, _cx| {
                        editor.document_mut().save_as(path.clone())
                    });
                    match result {
                        Ok(()) => {
                            let name = editor.read(cx).document().display_name();
                            workspace.set_status(format!("Saved {name}"), cx);
                        }
                        Err(error) => workspace.set_status(error.to_string(), cx),
                    }
                });
            }
        })
        .detach();
    }

    /// Closes the active tab, prompting when it has unsaved changes.
    pub fn perform_close_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_tab_at(self.active, window, cx);
    }

    /// Activates the next tab, wrapping around.
    pub fn next_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.tabs.len() > 1 {
            self.active = (self.active + 1) % self.tabs.len();
            self.focus_active_editor(window, cx);
            cx.notify();
        }
    }

    /// Activates the previous tab, wrapping around.
    pub fn previous_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.tabs.len() > 1 {
            self.active = (self.active + self.tabs.len() - 1) % self.tabs.len();
            self.focus_active_editor(window, cx);
            cx.notify();
        }
    }

    // ---------------------------------------------------------------
    // Action handlers
    // ---------------------------------------------------------------

    fn on_new_file(&mut self, _: &NewFile, window: &mut Window, cx: &mut Context<Self>) {
        self.perform_new_file(window, cx);
    }

    fn on_open_file(&mut self, _: &OpenFile, window: &mut Window, cx: &mut Context<Self>) {
        self.perform_open_file(window, cx);
    }

    fn on_save_file(&mut self, _: &SaveFile, window: &mut Window, cx: &mut Context<Self>) {
        if self.active_editor().read(cx).document().path().is_some() {
            self.perform_save(cx);
        } else {
            self.perform_save_as(window, cx);
        }
    }

    fn on_save_file_as(&mut self, _: &SaveFileAs, window: &mut Window, cx: &mut Context<Self>) {
        self.perform_save_as(window, cx);
    }

    fn on_close_tab(&mut self, _: &CloseTab, window: &mut Window, cx: &mut Context<Self>) {
        self.perform_close_tab(window, cx);
    }

    fn on_next_tab(&mut self, _: &NextTab, window: &mut Window, cx: &mut Context<Self>) {
        self.next_tab(window, cx);
    }

    fn on_previous_tab(&mut self, _: &PreviousTab, window: &mut Window, cx: &mut Context<Self>) {
        self.previous_tab(window, cx);
    }

    fn on_undo(&mut self, _: &UndoEdit, _window: &mut Window, cx: &mut Context<Self>) {
        self.with_active_editor(cx, |editor, cx| editor.undo(cx));
    }

    fn on_redo(&mut self, _: &RedoEdit, _window: &mut Window, cx: &mut Context<Self>) {
        self.with_active_editor(cx, |editor, cx| editor.redo(cx));
    }

    fn on_select_all(&mut self, _: &SelectAllText, _window: &mut Window, cx: &mut Context<Self>) {
        self.with_active_editor(cx, |editor, cx| editor.select_all(cx));
    }

    fn on_cut(&mut self, _: &CutSelection, _window: &mut Window, cx: &mut Context<Self>) {
        self.with_active_editor(cx, |editor, cx| editor.cut(cx));
    }

    fn on_copy(&mut self, _: &CopySelection, _window: &mut Window, cx: &mut Context<Self>) {
        self.with_active_editor(cx, |editor, cx| editor.copy(cx));
    }

    fn on_paste(&mut self, _: &PasteClipboard, _window: &mut Window, cx: &mut Context<Self>) {
        self.with_active_editor(cx, |editor, cx| editor.paste(cx));
    }

    fn on_move_left(&mut self, _: &MoveLeft, _window: &mut Window, cx: &mut Context<Self>) {
        self.with_active_editor(cx, |editor, cx| editor.move_horizontal(false, false, cx));
    }

    fn on_move_right(&mut self, _: &MoveRight, _window: &mut Window, cx: &mut Context<Self>) {
        self.with_active_editor(cx, |editor, cx| editor.move_horizontal(true, false, cx));
    }

    fn on_move_up(&mut self, _: &MoveUp, _window: &mut Window, cx: &mut Context<Self>) {
        if self.palette.open {
            self.palette.select_previous();
            cx.notify();
        } else {
            self.with_active_editor(cx, |editor, cx| editor.move_vertical(false, false, cx));
        }
    }

    fn on_move_down(&mut self, _: &MoveDown, _window: &mut Window, cx: &mut Context<Self>) {
        if self.palette.open {
            self.palette.select_next();
            cx.notify();
        } else {
            self.with_active_editor(cx, |editor, cx| editor.move_vertical(true, false, cx));
        }
    }

    fn on_extend_left(&mut self, _: &ExtendLeft, _window: &mut Window, cx: &mut Context<Self>) {
        self.with_active_editor(cx, |editor, cx| editor.move_horizontal(false, true, cx));
    }

    fn on_extend_right(&mut self, _: &ExtendRight, _window: &mut Window, cx: &mut Context<Self>) {
        self.with_active_editor(cx, |editor, cx| editor.move_horizontal(true, true, cx));
    }

    fn on_extend_up(&mut self, _: &ExtendUp, _window: &mut Window, cx: &mut Context<Self>) {
        self.with_active_editor(cx, |editor, cx| editor.move_vertical(false, true, cx));
    }

    fn on_extend_down(&mut self, _: &ExtendDown, _window: &mut Window, cx: &mut Context<Self>) {
        self.with_active_editor(cx, |editor, cx| editor.move_vertical(true, true, cx));
    }

    fn on_move_word_left(
        &mut self,
        _: &MoveWordLeft,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.with_active_editor(cx, |editor, cx| editor.move_word(false, false, cx));
    }

    fn on_move_word_right(
        &mut self,
        _: &MoveWordRight,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.with_active_editor(cx, |editor, cx| editor.move_word(true, false, cx));
    }

    fn on_extend_word_left(
        &mut self,
        _: &ExtendWordLeft,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.with_active_editor(cx, |editor, cx| editor.move_word(false, true, cx));
    }

    fn on_extend_word_right(
        &mut self,
        _: &ExtendWordRight,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.with_active_editor(cx, |editor, cx| editor.move_word(true, true, cx));
    }

    fn on_indent(&mut self, _: &Indent, _window: &mut Window, cx: &mut Context<Self>) {
        self.with_active_editor(cx, |editor, cx| editor.indent(false, cx));
    }

    fn on_outdent(&mut self, _: &Outdent, _window: &mut Window, cx: &mut Context<Self>) {
        self.with_active_editor(cx, |editor, cx| editor.indent(true, cx));
    }

    fn on_move_line_start(
        &mut self,
        _: &MoveLineStart,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.with_active_editor(cx, |editor, cx| editor.move_line_start(false, cx));
    }

    fn on_move_line_end(&mut self, _: &MoveLineEnd, _window: &mut Window, cx: &mut Context<Self>) {
        self.with_active_editor(cx, |editor, cx| editor.move_line_end(false, cx));
    }

    fn on_move_document_start(
        &mut self,
        _: &MoveDocumentStart,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.with_active_editor(cx, |editor, cx| editor.move_document_start(false, cx));
    }

    fn on_move_document_end(
        &mut self,
        _: &MoveDocumentEnd,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.with_active_editor(cx, |editor, cx| editor.move_document_end(false, cx));
    }

    fn on_delete_backward(
        &mut self,
        _: &DeleteBackward,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.palette.open {
            self.palette.backspace();
            cx.notify();
        } else {
            self.with_active_editor(cx, |editor, cx| editor.delete_backward(cx));
        }
    }

    fn on_delete_forward(
        &mut self,
        _: &DeleteForward,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.with_active_editor(cx, |editor, cx| editor.delete_forward(cx));
    }

    fn on_insert_newline(
        &mut self,
        _: &InsertNewline,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.palette.open {
            self.run_selected_palette_command(window, cx);
        } else {
            self.with_active_editor(cx, |editor, cx| editor.insert_newline(cx));
        }
    }

    fn on_toggle_palette(
        &mut self,
        _: &TogglePalette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.palette.open {
            self.close_palette(window, cx);
        } else {
            self.palette.open();
            window.focus(&self.palette.focus_handle);
            cx.notify();
        }
    }

    fn on_dismiss(&mut self, _: &Dismiss, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette.open {
            self.close_palette(window, cx);
        } else {
            self.with_active_editor(cx, |editor, cx| editor.clear_selection(cx));
        }
    }

    fn on_quit(&mut self, _: &QuitEditor, _window: &mut Window, cx: &mut Context<Self>) {
        cx.quit();
    }

    // ---------------------------------------------------------------
    // Palette
    // ---------------------------------------------------------------

    fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.palette.close();
        self.focus_active_editor(window, cx);
        cx.notify();
    }

    fn palette_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.palette.handle_key_down(event) {
            cx.notify();
        }
    }

    fn run_selected_palette_command(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(command) = self.palette.selected_command() {
            self.run_palette_command(command, window, cx);
        }
    }

    fn run_palette_command(
        &mut self,
        command: PaletteCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_palette(window, cx);
        match command {
            PaletteCommand::NewFile => self.perform_new_file(window, cx),
            PaletteCommand::OpenFile => self.perform_open_file(window, cx),
            PaletteCommand::SaveFile => self.perform_save(cx),
            PaletteCommand::SaveFileAs => self.perform_save_as(window, cx),
            PaletteCommand::CloseTab => self.perform_close_tab(window, cx),
            PaletteCommand::NextTab => self.next_tab(window, cx),
            PaletteCommand::PreviousTab => self.previous_tab(window, cx),
            PaletteCommand::Undo => self.with_active_editor(cx, |editor, cx| editor.undo(cx)),
            PaletteCommand::Redo => self.with_active_editor(cx, |editor, cx| editor.redo(cx)),
            PaletteCommand::SelectAll => {
                self.with_active_editor(cx, |editor, cx| editor.select_all(cx));
            }
            PaletteCommand::Quit => cx.quit(),
        }
    }

    // ---------------------------------------------------------------
    // Mouse handling
    // ---------------------------------------------------------------

    fn editor_mouse_down(
        &mut self,
        editor: &Entity<EditorView>,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.palette.open {
            return;
        }
        if let Some((_line, byte)) = editor.read(cx).hit_test(event.position) {
            let extend = event.modifiers.shift;
            let click_count = event.click_count;
            editor.update(cx, |editor, cx| {
                editor.begin_selection(byte, extend, click_count, cx);
            });
        }
        let handle = editor.read(cx).focus_handle().clone();
        window.focus(&handle);
    }

    fn editor_mouse_move(
        &mut self,
        editor: &Entity<EditorView>,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.pressed_button != Some(MouseButton::Left) {
            return;
        }
        if !editor.read(cx).dragging() {
            return;
        }
        if let Some((_line, byte)) = editor.read(cx).hit_test(event.position) {
            editor.update(cx, |editor, cx| editor.extend_selection(byte, cx));
        }
    }

    fn editor_mouse_up(
        &mut self,
        editor: &Entity<EditorView>,
        _event: &MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        editor.update(cx, |editor, cx| editor.end_drag(cx));
    }

    // ---------------------------------------------------------------
    // Tabs
    // ---------------------------------------------------------------

    fn activate_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index < self.tabs.len() {
            self.active = index;
            self.focus_active_editor(window, cx);
            cx.notify();
        }
    }

    fn close_tab_at(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }
        let editor = self.tabs[index].clone();
        if editor.read(cx).document().is_dirty() {
            let name = editor.read(cx).document().display_name();
            let receiver = window.prompt(
                PromptLevel::Warning,
                "Discard unsaved changes?",
                Some(&name),
                &["Discard", "Cancel"],
                cx,
            );
            cx.spawn_in(window, async move |this, cx| {
                if let Ok(0) = receiver.await {
                    let _ = this.update_in(cx, |workspace, window, cx| {
                        workspace.close_tab_index_now(index, window, cx);
                    });
                }
            })
            .detach();
        } else {
            self.close_tab_index_now(index, window, cx);
        }
    }

    fn close_tab_index_now(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }
        if self.tabs.len() == 1 {
            let editor = self.new_editor(Document::untitled(), cx);
            self.tabs[0] = editor;
            self.active = 0;
        } else {
            self.tabs.remove(index);
            if self.active >= self.tabs.len() {
                self.active = self.tabs.len() - 1;
            } else if index < self.active {
                self.active -= 1;
            }
        }
        self.focus_active_editor(window, cx);
        cx.notify();
    }

    // ---------------------------------------------------------------
    // Helpers
    // ---------------------------------------------------------------

    fn new_editor(&mut self, document: Document, cx: &mut Context<Self>) -> Entity<EditorView> {
        let theme = self.theme.clone();
        let editor = cx.new(|cx| EditorView::new(document, theme, cx));
        cx.observe(&editor, |_, _, cx| cx.notify()).detach();
        editor
    }

    fn focus_active_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let handle: FocusHandle = self.active_editor().read(cx).focus_handle().clone();
        window.focus(&handle);
    }

    fn with_active_editor(
        &self,
        cx: &mut Context<Self>,
        update: impl FnOnce(&mut EditorView, &mut Context<EditorView>),
    ) {
        if self.palette.open {
            return;
        }
        let editor = self.active_editor().clone();
        editor.update(cx, |editor, cx| update(editor, cx));
    }

    fn set_status(&mut self, message: String, cx: &mut Context<Self>) {
        self.status_message = Some(SharedString::from(message));
        cx.notify();
    }

    fn set_window_title(&mut self, window: &mut Window, cx: &Context<Self>) {
        let editor = self.active_editor().read(cx);
        let name = editor.document().display_name();
        let dirty = if editor.document().is_dirty() {
            " •"
        } else {
            ""
        };
        window.set_window_title(&format!("{name}{dirty} — Stet"));
    }

    // ---------------------------------------------------------------
    // Rendering
    // ---------------------------------------------------------------

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = self.theme.clone();
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_3()
            .h(px(38.))
            .flex_shrink_0()
            .bg(theme.surface)
            .border_b_1()
            .border_color(theme.border)
            .child(div().text_sm().text_color(theme.muted).child("Stet"))
            .child(div().flex_1())
            .child(
                div()
                    .id("toolbar-open")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .text_sm()
                    .cursor_pointer()
                    .hover(|this| this.bg(theme.surface_raised))
                    .on_click(cx.listener(|workspace, _: &ClickEvent, window, cx| {
                        workspace.perform_open_file(window, cx);
                    }))
                    .child("Open"),
            )
            .child(
                div()
                    .id("toolbar-save")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .text_sm()
                    .cursor_pointer()
                    .hover(|this| this.bg(theme.surface_raised))
                    .on_click(cx.listener(|workspace, _: &ClickEvent, window, cx| {
                        workspace.on_save_file(&SaveFile, window, cx);
                    }))
                    .child("Save"),
            )
            .child(
                div()
                    .id("toolbar-palette")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .text_sm()
                    .cursor_pointer()
                    .hover(|this| this.bg(theme.surface_raised))
                    .on_click(cx.listener(|workspace, _: &ClickEvent, window, cx| {
                        workspace.on_toggle_palette(&TogglePalette, window, cx);
                    }))
                    .child("Commands"),
            )
    }

    fn render_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = self.theme.clone();
        let tabs: Vec<_> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(index, editor)| {
                let active = index == self.active;
                let (title, dirty) = {
                    let editor = editor.read(cx);
                    (
                        editor.document().display_name(),
                        editor.document().is_dirty(),
                    )
                };
                div()
                    .id(("tab", editor.entity_id()))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .px_3()
                    .h_full()
                    .text_sm()
                    .cursor_pointer()
                    .bg(if active {
                        theme.surface_raised
                    } else {
                        theme.surface
                    })
                    .text_color(if active {
                        theme.foreground
                    } else {
                        theme.muted
                    })
                    .border_r_1()
                    .border_color(theme.border)
                    .on_click(cx.listener(move |workspace, _: &ClickEvent, window, cx| {
                        workspace.activate_tab(index, window, cx);
                    }))
                    .child(SharedString::from(title))
                    .when(dirty, |this| {
                        this.child(div().text_color(theme.accent).child("•"))
                    })
                    .child(
                        div()
                            .id(("tab-close", editor.entity_id()))
                            .px_1()
                            .rounded_md()
                            .hover(|this| this.bg(theme.border))
                            .on_click(cx.listener(move |workspace, _: &ClickEvent, window, cx| {
                                cx.stop_propagation();
                                workspace.close_tab_at(index, window, cx);
                            }))
                            .child("×"),
                    )
            })
            .collect();

        div()
            .flex()
            .flex_row()
            .items_center()
            .h(px(34.))
            .flex_shrink_0()
            .bg(theme.surface)
            .border_b_1()
            .border_color(theme.border)
            .children(tabs)
    }

    fn render_editor(
        &self,
        editor: Entity<EditorView>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let line_count = editor.read(cx).line_count();
        let scroll_handle = editor.read(cx).scroll_handle().clone();
        let focus_handle = editor.read(cx).focus_handle().clone();

        let editor_input = editor.clone();
        let editor_down = editor.clone();
        let editor_move = editor.clone();
        let editor_up = editor.clone();
        let editor_items = editor.clone();

        div()
            .relative()
            .flex()
            .flex_row()
            .flex_1()
            .overflow_hidden()
            .font_family(EDITOR_FONT)
            .text_size(px(crate::editor::FONT_SIZE))
            .track_focus(&focus_handle)
            .cursor(gpui::CursorStyle::IBeam)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |workspace, event: &MouseDownEvent, window, cx| {
                    workspace.editor_mouse_down(&editor_down, event, window, cx);
                }),
            )
            .on_mouse_move(
                cx.listener(move |workspace, event: &MouseMoveEvent, window, cx| {
                    workspace.editor_mouse_move(&editor_move, event, window, cx);
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |workspace, event: &MouseUpEvent, window, cx| {
                    workspace.editor_mouse_up(&editor_up, event, window, cx);
                }),
            )
            .child(
                // Registers the focused editor as the window's text input
                // handler; it paints nothing and takes no hitbox.
                div()
                    .absolute()
                    .top(px(0.))
                    .left(px(0.))
                    .w_full()
                    .h_full()
                    .child(InputAreaElement {
                        editor: editor_input,
                    }),
            )
            .child(
                uniform_list("editor-lines", line_count, move |range, _window, cx| {
                    editor_items.update(cx, |editor, cx| {
                        editor.set_visible_lines(range.clone());
                        let theme = editor.theme().clone();
                        let cursor_line = editor.cursor_line();
                        let editor_entity = cx.entity();
                        range
                            .map(|line| {
                                div()
                                    .flex()
                                    .flex_row()
                                    .w_full()
                                    .h(px(LINE_HEIGHT))
                                    .child(
                                        div()
                                            .w(px(GUTTER_WIDTH))
                                            .h(px(LINE_HEIGHT))
                                            .flex()
                                            .justify_end()
                                            .items_center()
                                            .pr(px(10.))
                                            .text_size(px(11.))
                                            .text_color(if line == cursor_line {
                                                theme.gutter_active
                                            } else {
                                                theme.gutter
                                            })
                                            .child((line + 1).to_string()),
                                    )
                                    .child(
                                        div()
                                            .flex_grow()
                                            .h(px(LINE_HEIGHT))
                                            .overflow_hidden()
                                            .child(LineElement {
                                                editor: editor_entity.clone(),
                                                line,
                                            }),
                                    )
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .h_full()
                .track_scroll(scroll_handle),
            )
    }

    fn render_status_bar(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let theme = self.theme.clone();
        let editor = self.active_editor().read(cx);
        let (line, column) = editor.cursor_status();
        let language = editor.language().display_name();
        let name = editor.document().display_name();

        let mut bar = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px_3()
            .h(px(26.))
            .flex_shrink_0()
            .bg(theme.surface)
            .border_t_1()
            .border_color(theme.border)
            .text_size(px(12.))
            .text_color(theme.muted)
            .child(SharedString::from(name))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_3()
                    .child(SharedString::from(language.to_string()))
                    .child(SharedString::from(format!("Ln {line}, Col {column}"))),
            );

        if let Some(message) = self.status_message.clone() {
            bar = bar.child(div().text_color(theme.foreground).child(message));
        }

        bar
    }

    fn render_palette(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = self.theme.clone();
        let commands = self.palette.filtered();

        let empty = commands.is_empty();
        let mut list = div().flex().flex_col().max_h(px(320.)).overflow_hidden();
        if empty {
            list = list.child(
                div()
                    .px_3()
                    .py_2()
                    .text_sm()
                    .text_color(theme.muted)
                    .child("No matching commands"),
            );
        } else {
            list = list.children(commands.into_iter().enumerate().map(|(index, command)| {
                let selected = index == self.palette.selected;
                div()
                    .id(ElementId::named_usize("palette-item", index))
                    .px_3()
                    .py_1()
                    .text_sm()
                    .cursor_pointer()
                    .when(selected, |this| this.bg(theme.accent.opacity(0.22)))
                    .hover(|this| this.bg(theme.accent.opacity(0.15)))
                    .on_click(cx.listener(move |workspace, _: &ClickEvent, window, cx| {
                        workspace.run_palette_command(command, window, cx);
                    }))
                    .child(SharedString::from(command.title()))
            }));
        }

        div()
            .absolute()
            .top(px(0.))
            .left(px(0.))
            .right(px(0.))
            .bottom(px(0.))
            .flex()
            .flex_col()
            .items_center()
            .bg(rgba(0x00000055))
            .occlude()
            .child(
                div()
                    .mt(px(72.))
                    .w(px(560.))
                    .flex()
                    .flex_col()
                    .bg(theme.surface_raised)
                    .text_color(theme.foreground)
                    .border_1()
                    .border_color(theme.border)
                    .rounded_md()
                    .shadow_lg()
                    .track_focus(&self.palette.focus_handle)
                    .on_key_down(cx.listener(Self::palette_key_down))
                    .on_mouse_down_out(cx.listener(|workspace, _: &MouseDownEvent, window, cx| {
                        workspace.close_palette(window, cx);
                    }))
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .text_sm()
                            .border_b_1()
                            .border_color(theme.border)
                            .child(if self.palette.query.is_empty() {
                                SharedString::from("Type a command…")
                            } else {
                                SharedString::from(self.palette.query.clone())
                            }),
                    )
                    .child(list),
            )
    }
}

impl gpui::Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.set_window_title(window, cx);

        let theme = self.theme.clone();
        let editor = self.active_editor().clone();
        let editor_area = self.render_editor(editor, cx);
        let palette = self.palette.open.then(|| self.render_palette(cx));

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .on_action(cx.listener(Self::on_new_file))
            .on_action(cx.listener(Self::on_open_file))
            .on_action(cx.listener(Self::on_save_file))
            .on_action(cx.listener(Self::on_save_file_as))
            .on_action(cx.listener(Self::on_close_tab))
            .on_action(cx.listener(Self::on_next_tab))
            .on_action(cx.listener(Self::on_previous_tab))
            .on_action(cx.listener(Self::on_undo))
            .on_action(cx.listener(Self::on_redo))
            .on_action(cx.listener(Self::on_select_all))
            .on_action(cx.listener(Self::on_cut))
            .on_action(cx.listener(Self::on_copy))
            .on_action(cx.listener(Self::on_paste))
            .on_action(cx.listener(Self::on_move_left))
            .on_action(cx.listener(Self::on_move_right))
            .on_action(cx.listener(Self::on_move_up))
            .on_action(cx.listener(Self::on_move_down))
            .on_action(cx.listener(Self::on_extend_left))
            .on_action(cx.listener(Self::on_extend_right))
            .on_action(cx.listener(Self::on_extend_up))
            .on_action(cx.listener(Self::on_extend_down))
            .on_action(cx.listener(Self::on_move_word_left))
            .on_action(cx.listener(Self::on_move_word_right))
            .on_action(cx.listener(Self::on_extend_word_left))
            .on_action(cx.listener(Self::on_extend_word_right))
            .on_action(cx.listener(Self::on_indent))
            .on_action(cx.listener(Self::on_outdent))
            .on_action(cx.listener(Self::on_move_line_start))
            .on_action(cx.listener(Self::on_move_line_end))
            .on_action(cx.listener(Self::on_move_document_start))
            .on_action(cx.listener(Self::on_move_document_end))
            .on_action(cx.listener(Self::on_delete_backward))
            .on_action(cx.listener(Self::on_delete_forward))
            .on_action(cx.listener(Self::on_insert_newline))
            .on_action(cx.listener(Self::on_toggle_palette))
            .on_action(cx.listener(Self::on_dismiss))
            .on_action(cx.listener(Self::on_quit))
            .child(self.render_toolbar(cx))
            .child(self.render_tabs(cx))
            .child(editor_area)
            .child(self.render_status_bar(cx))
            .children(palette)
    }
}
