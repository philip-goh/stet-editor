//! Editor actions, bound to keys in [`crate::app`] and dispatched by the
//! workspace.
//!
//! Actions are the single entry point for every command: keyboard
//! shortcuts, the command palette and toolbar buttons all invoke the same
//! handler, so behaviour cannot drift between input methods.

use gpui::actions;

actions!(
    stet,
    [
        /// Creates a new untitled document.
        NewFile,
        /// Opens the file picker.
        OpenFile,
        /// Saves the active document.
        SaveFile,
        /// Saves the active document to a new path.
        SaveFileAs,
        /// Closes the active tab.
        CloseTab,
        /// Activates the next tab.
        NextTab,
        /// Activates the previous tab.
        PreviousTab,
        /// Undoes the most recent edit.
        UndoEdit,
        /// Redoes the most recently undone edit.
        RedoEdit,
        /// Selects the entire document.
        SelectAllText,
        /// Cuts the selection to the clipboard.
        CutSelection,
        /// Copies the selection to the clipboard.
        CopySelection,
        /// Pastes the clipboard over the selection.
        PasteClipboard,
        /// Moves the caret one grapheme left.
        MoveLeft,
        /// Moves the caret one grapheme right.
        MoveRight,
        /// Moves the caret one line up.
        MoveUp,
        /// Moves the caret one line down.
        MoveDown,
        /// Extends the selection one grapheme left.
        ExtendLeft,
        /// Extends the selection one grapheme right.
        ExtendRight,
        /// Extends the selection one line up.
        ExtendUp,
        /// Extends the selection one line down.
        ExtendDown,
        /// Moves the caret one word left.
        MoveWordLeft,
        /// Moves the caret one word right.
        MoveWordRight,
        /// Extends the selection one word left.
        ExtendWordLeft,
        /// Extends the selection one word right.
        ExtendWordRight,
        /// Indents the current line or selected lines.
        Indent,
        /// Outdents the current line or selected lines.
        Outdent,
        /// Moves the caret to the start of the line.
        MoveLineStart,
        /// Moves the caret to the end of the line.
        MoveLineEnd,
        /// Moves the caret to the start of the document.
        MoveDocumentStart,
        /// Moves the caret to the end of the document.
        MoveDocumentEnd,
        /// Deletes the previous grapheme or the selection.
        DeleteBackward,
        /// Deletes the next grapheme or the selection.
        DeleteForward,
        /// Inserts a newline, or accepts a command palette item.
        InsertNewline,
        /// Toggles the command palette.
        TogglePalette,
        /// Opens the find bar.
        OpenFind,
        /// Opens the find bar with the replace row.
        OpenFindReplace,
        /// Moves to the previous find match.
        FindPrevious,
        /// Toggles case sensitivity in the find bar.
        ToggleFindCase,
        /// Dismisses the command palette or clears the selection.
        Dismiss,
        /// Quits the editor.
        QuitEditor,
    ]
);
