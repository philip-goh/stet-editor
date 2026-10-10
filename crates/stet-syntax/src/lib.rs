//! Language detection and syntax highlighting for the Stet editor.
//!
//! Highlighting is tree-sitter based (see
//! `docs/adr/0005-syntax-highlighting.md`). The public surface is
//! deliberately small:
//!
//! - [`Language`] maps file paths and extensions to a grammar.
//! - [`Highlighter`] owns the parse state for one document and answers
//!   "what spans on line N?" queries for the UI.
//! - [`HighlightSpan`] and [`HighlightKind`] describe styled byte ranges
//!   relative to the start of a line.
//!
//! Highlighting is a pure function of the text. The UI can rebuild spans on
//! every change without worrying about stale state; the engine simply keeps
//! a per-line cache to avoid re-bucketing spans on every frame.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod highlight;
mod language;

pub use highlight::{HighlightKind, HighlightSpan, Highlighter, MAX_HIGHLIGHT_BYTES};
pub use language::Language;
