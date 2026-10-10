//! State for the find and replace bar.
//!
//! Rendering and event wiring live in the workspace; this type holds the
//! data: the query, replacement, which field is focused, the latest search
//! result and the current match. The actual matching is in
//! [`stet_core::find_matches`], which is independently tested.

use std::ops::Range;

use gpui::FocusHandle;
use stet_core::{SearchOptions, SearchResult};

/// Which find-bar field receives typed characters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FindField {
    /// The search query field.
    Query,
    /// The replacement text field.
    Replace,
}

/// The find bar's UI state.
pub struct FindState {
    /// Whether the bar is visible.
    pub open: bool,
    /// Whether the replace field and buttons are shown.
    pub replace_mode: bool,
    /// The search query.
    pub query: String,
    /// The replacement text.
    pub replacement: String,
    /// Which field is focused.
    pub field: FindField,
    /// Whether matching is case-sensitive.
    pub case_sensitive: bool,
    /// The latest search result.
    pub matches: SearchResult,
    /// Index of the current match within [`FindState::matches`].
    pub current: Option<usize>,
    /// The editor text revision the matches were computed from.
    pub revision: u64,
    /// Focus handle for the query field.
    pub query_focus: FocusHandle,
    /// Focus handle for the replace field.
    pub replace_focus: FocusHandle,
}

impl FindState {
    /// Creates a hidden find bar.
    pub fn new(query_focus: FocusHandle, replace_focus: FocusHandle) -> Self {
        Self {
            open: false,
            replace_mode: false,
            query: String::new(),
            replacement: String::new(),
            field: FindField::Query,
            case_sensitive: false,
            matches: SearchResult::default(),
            current: None,
            revision: 0,
            query_focus,
            replace_focus,
        }
    }

    /// Shows the bar, optionally with the replace row, and picks the field
    /// to focus.
    pub fn show(&mut self, replace_mode: bool) {
        self.open = true;
        self.replace_mode = replace_mode;
        self.field = if replace_mode {
            FindField::Replace
        } else {
            FindField::Query
        };
    }

    /// Hides the bar and forgets the search state.
    pub fn hide(&mut self) {
        self.open = false;
        self.replace_mode = false;
        self.field = FindField::Query;
        self.matches = SearchResult::default();
        self.current = None;
        self.revision = 0;
    }

    /// Returns the search options for the core matcher.
    pub fn options(&self) -> SearchOptions {
        SearchOptions {
            case_sensitive: self.case_sensitive,
        }
    }

    /// Returns the focus handle of the active field.
    pub fn field_focus(&self) -> &FocusHandle {
        match self.field {
            FindField::Query => &self.query_focus,
            FindField::Replace => &self.replace_focus,
        }
    }

    /// Appends typed text to the active field.
    ///
    /// Returns `true` if the query changed (and the search must be rerun).
    pub fn push_str(&mut self, text: &str) -> bool {
        match self.field {
            FindField::Query => {
                self.query.push_str(text);
                true
            }
            FindField::Replace => {
                self.replacement.push_str(text);
                false
            }
        }
    }

    /// Removes the last character of the active field.
    ///
    /// Returns `true` if the query changed.
    pub fn backspace(&mut self) -> bool {
        match self.field {
            FindField::Query => {
                self.query.pop();
                true
            }
            FindField::Replace => {
                self.replacement.pop();
                false
            }
        }
    }

    /// Toggles case sensitivity, returning `true` since the search changes.
    pub fn toggle_case(&mut self) -> bool {
        self.case_sensitive = !self.case_sensitive;
        true
    }

    /// Returns the byte range of the current match, if any.
    pub fn current_range(&self) -> Option<Range<usize>> {
        self.current
            .and_then(|index| self.matches.matches.get(index).cloned())
    }

    /// Returns the "3 of 17" style label for the bar.
    pub fn match_label(&self) -> String {
        if self.query.is_empty() {
            return String::new();
        }
        match self.current {
            Some(index) => {
                let suffix = if self.matches.truncated { "+" } else { "" };
                format!("{} of {}{}", index + 1, self.matches.len(), suffix)
            }
            None => "No results".to_owned(),
        }
    }
}
