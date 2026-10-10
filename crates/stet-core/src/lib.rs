//! UI-agnostic text engine for the Stet editor.
//!
//! This crate is the single source of truth for document state. It knows
//! nothing about rendering, input handling or GPUI (see
//! `docs/adr/0002-editor-core-is-ui-agnostic.md`), which keeps the logic
//! testable without a display server and makes it possible to swap the UI
//! layer later.
//!
//! Key types:
//!
//! - [`TextBuffer`]: a rope-backed, editable sequence of UTF-8 text with
//!   position conversion and grapheme-aware navigation helpers.
//! - [`Document`]: a [`TextBuffer`] paired with a file path, dirty tracking
//!   and an undo/redo [`History`].
//! - [`Selection`]: an anchor/head pair of byte offsets.
//! - [`Position`]: a zero-based line/column pair, where columns count
//!   Unicode scalar values.
//!
//! All offsets exposed by this crate are **byte offsets into the buffer**
//! unless explicitly stated otherwise. Mixing byte and character units is
//! the classic editor bug factory, so the unit is always part of the name.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod document;
mod edit;
mod error;
mod history;
mod position;
mod search;
mod selection;
mod text;

pub use document::Document;
pub use edit::{AppliedEdit, EditKind, EditSummary, Point};
pub use error::{Error, Result};
pub use history::{History, UndoStep};
pub use position::Position;
pub use search::{MAX_MATCHES, SearchOptions, SearchResult, find_matches};
pub use selection::{Replacement, Selection, SelectionSet, map_ranges_through_edits};
pub use text::{LineEnding, TextBuffer};
