//! State for the command palette.

use gpui::{FocusHandle, KeyDownEvent};

/// A command the palette can run.
///
/// The palette is deliberately a fixed list of editor commands in v0.1.
/// Plugin-contributed commands will be merged into this list when the
/// plugin host lands (see `docs/adr/0006-plugin-system.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteCommand {
    /// New untitled document.
    NewFile,
    /// Open a file.
    OpenFile,
    /// Save.
    SaveFile,
    /// Save as.
    SaveFileAs,
    /// Close the active tab.
    CloseTab,
    /// Activate the next tab.
    NextTab,
    /// Activate the previous tab.
    PreviousTab,
    /// Undo.
    Undo,
    /// Redo.
    Redo,
    /// Select all.
    SelectAll,
    /// Quit.
    Quit,
}

impl PaletteCommand {
    /// All commands, in palette order.
    pub const ALL: &'static [PaletteCommand] = &[
        PaletteCommand::NewFile,
        PaletteCommand::OpenFile,
        PaletteCommand::SaveFile,
        PaletteCommand::SaveFileAs,
        PaletteCommand::CloseTab,
        PaletteCommand::NextTab,
        PaletteCommand::PreviousTab,
        PaletteCommand::Undo,
        PaletteCommand::Redo,
        PaletteCommand::SelectAll,
        PaletteCommand::Quit,
    ];

    /// The title shown in the palette.
    pub fn title(self) -> &'static str {
        match self {
            PaletteCommand::NewFile => "File: New",
            PaletteCommand::OpenFile => "File: Open…",
            PaletteCommand::SaveFile => "File: Save",
            PaletteCommand::SaveFileAs => "File: Save As…",
            PaletteCommand::CloseTab => "View: Close Tab",
            PaletteCommand::NextTab => "View: Next Tab",
            PaletteCommand::PreviousTab => "View: Previous Tab",
            PaletteCommand::Undo => "Edit: Undo",
            PaletteCommand::Redo => "Edit: Redo",
            PaletteCommand::SelectAll => "Edit: Select All",
            PaletteCommand::Quit => "Application: Quit",
        }
    }
}

/// The command palette's UI state.
///
/// Rendering lives in the workspace so that mouse and key listeners can be
/// workspace-entity listeners; this type holds only the data.
pub struct Palette {
    /// Whether the palette is visible.
    pub open: bool,
    /// The current filter text.
    pub query: String,
    /// The index of the highlighted item in the filtered list.
    pub selected: usize,
    /// Focus handle for the palette overlay.
    pub focus_handle: FocusHandle,
}

impl Palette {
    /// Creates a closed palette.
    pub fn new(focus_handle: FocusHandle) -> Self {
        Self {
            open: false,
            query: String::new(),
            selected: 0,
            focus_handle,
        }
    }

    /// Opens (or re-opens) the palette with an empty query.
    pub fn open(&mut self) {
        self.open = true;
        self.query.clear();
        self.selected = 0;
    }

    /// Closes the palette.
    pub fn close(&mut self) {
        self.open = false;
        self.query.clear();
        self.selected = 0;
    }

    /// Returns the commands matching the current query, in display order.
    ///
    /// Matching is a case-insensitive substring test, which is predictable
    /// and cheap; something fuzzier can come later without changing
    /// callers.
    pub fn filtered(&self) -> Vec<PaletteCommand> {
        let query = self.query.to_lowercase();
        PaletteCommand::ALL
            .iter()
            .copied()
            .filter(|command| query.is_empty() || command.title().to_lowercase().contains(&query))
            .collect()
    }

    /// Moves the highlight down one item, clamping at the end.
    pub fn select_next(&mut self) {
        let count = self.filtered().len();
        if count > 0 {
            self.selected = (self.selected + 1).min(count - 1);
        }
    }

    /// Moves the highlight up one item, clamping at the start.
    pub fn select_previous(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    /// Backspaces the query, keeping the highlight in range.
    pub fn backspace(&mut self) {
        self.query.pop();
        let count = self.filtered().len();
        if self.selected >= count {
            self.selected = count.saturating_sub(1);
        }
    }

    /// Appends typed text to the query and resets the highlight.
    pub fn push_str(&mut self, text: &str) {
        self.query.push_str(text);
        self.selected = 0;
    }

    /// Returns the currently highlighted command, if any.
    pub fn selected_command(&self) -> Option<PaletteCommand> {
        self.filtered().get(self.selected).copied()
    }

    /// Handles a key-down event, appending printable characters to the
    /// query.
    ///
    /// Named keys (arrows, enter, backspace, escape) arrive as bound
    /// actions and are handled by the workspace instead.
    ///
    /// Returns `true` if the query changed.
    pub fn handle_key_down(&mut self, event: &KeyDownEvent) -> bool {
        let modifiers = event.keystroke.modifiers;
        if modifiers.control || modifiers.platform || modifiers.alt {
            return false;
        }
        let Some(text) = event.keystroke.key_char.as_deref() else {
            return false;
        };
        if text.is_empty() || text.chars().any(char::is_control) {
            return false;
        }
        self.push_str(text);
        true
    }
}
