//! Application startup and key bindings.

use std::path::PathBuf;

use gpui::prelude::*;
use gpui::{App, Application, Bounds, KeyBinding, WindowBounds, WindowOptions, px, size};

use crate::actions::*;
use crate::workspace::Workspace;

/// Launches the editor, optionally opening a file.
///
/// This blocks for the lifetime of the application; it must be called from
/// the main thread.
pub fn launch(initial_file: Option<PathBuf>) {
    Application::new().run(move |cx: &mut App| {
        cx.bind_keys(key_bindings());

        let bounds = Bounds::centered(None, size(px(1100.), px(720.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                focus: true,
                show: true,
                ..Default::default()
            },
            move |window, cx| {
                let workspace = cx.new(|cx| Workspace::new(initial_file.clone(), cx));
                let focus = workspace
                    .read(cx)
                    .active_editor()
                    .read(cx)
                    .focus_handle()
                    .clone();
                window.focus(&focus);
                workspace
            },
        )
        .expect("failed to open the Stet window");

        cx.activate(true);
    });
}

/// The default key bindings.
///
/// `cmd` in a binding means the platform's secondary modifier: Command on
/// macOS and Control on Windows and Linux.
fn key_bindings() -> Vec<KeyBinding> {
    vec![
        // Files and tabs.
        KeyBinding::new("cmd-n", NewFile, None),
        KeyBinding::new("cmd-o", OpenFile, None),
        KeyBinding::new("cmd-s", SaveFile, None),
        KeyBinding::new("cmd-shift-s", SaveFileAs, None),
        KeyBinding::new("cmd-w", CloseTab, None),
        KeyBinding::new("ctrl-tab", NextTab, None),
        KeyBinding::new("ctrl-shift-tab", PreviousTab, None),
        // Editing.
        KeyBinding::new("cmd-z", UndoEdit, None),
        KeyBinding::new("cmd-shift-z", RedoEdit, None),
        KeyBinding::new("cmd-a", SelectAllText, None),
        KeyBinding::new("cmd-x", CutSelection, None),
        KeyBinding::new("cmd-c", CopySelection, None),
        KeyBinding::new("cmd-v", PasteClipboard, None),
        // Caret movement.
        KeyBinding::new("left", MoveLeft, None),
        KeyBinding::new("right", MoveRight, None),
        KeyBinding::new("up", MoveUp, None),
        KeyBinding::new("down", MoveDown, None),
        KeyBinding::new("shift-left", ExtendLeft, None),
        KeyBinding::new("shift-right", ExtendRight, None),
        KeyBinding::new("shift-up", ExtendUp, None),
        KeyBinding::new("shift-down", ExtendDown, None),
        KeyBinding::new("alt-left", MoveWordLeft, None),
        KeyBinding::new("alt-right", MoveWordRight, None),
        KeyBinding::new("alt-shift-left", ExtendWordLeft, None),
        KeyBinding::new("alt-shift-right", ExtendWordRight, None),
        KeyBinding::new("home", MoveLineStart, None),
        KeyBinding::new("end", MoveLineEnd, None),
        KeyBinding::new("cmd-left", MoveLineStart, None),
        KeyBinding::new("cmd-right", MoveLineEnd, None),
        KeyBinding::new("cmd-up", MoveDocumentStart, None),
        KeyBinding::new("cmd-down", MoveDocumentEnd, None),
        // Multi-cursor.
        KeyBinding::new("cmd-d", SelectNextOccurrence, None),
        KeyBinding::new("cmd-alt-up", AddCursorAbove, None),
        KeyBinding::new("cmd-alt-down", AddCursorBelow, None),
        // Indentation.
        KeyBinding::new("tab", Indent, None),
        KeyBinding::new("shift-tab", Outdent, None),
        // Deletion and insertion.
        KeyBinding::new("backspace", DeleteBackward, None),
        KeyBinding::new("delete", DeleteForward, None),
        KeyBinding::new("enter", InsertNewline, None),
        // Application.
        KeyBinding::new("cmd-shift-p", TogglePalette, None),
        KeyBinding::new("cmd-f", OpenFind, None),
        KeyBinding::new("cmd-alt-f", OpenFindReplace, None),
        KeyBinding::new("shift-enter", FindPrevious, None),
        KeyBinding::new("cmd-alt-c", ToggleFindCase, None),
        KeyBinding::new("escape", Dismiss, None),
        KeyBinding::new("cmd-q", QuitEditor, None),
    ]
}
