//! GPUI-based user interface for the Stet editor.
//!
//! The UI layer is deliberately thin: document state, edits and undo live in
//! [`stet_core`], highlighting in [`stet_syntax`], and plugin contracts in
//! [`stet_plugin`]. This crate owns windows, input, and pixels only. See
//! `docs/architecture.md` for the full picture.
//!
//! Module map:
//!
//! - `app`: startup and key bindings.
//! - `workspace`: the application shell (tabs, toolbar, status bar, command
//!   palette, action routing).
//! - `editor`: the per-tab editor state and the custom line element.
//! - `palette`: command palette state.
//! - `theme`: colors.
//! - `actions`: the action vocabulary shared by keys, buttons and the
//!   palette.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod actions;
pub mod app;
pub mod editor;
pub mod find;
pub mod palette;
pub mod theme;
pub mod workspace;

pub use actions::*;
pub use app::launch;
pub use editor::EditorView;
pub use find::{FindField, FindState};
pub use palette::{Palette, PaletteCommand};
pub use theme::Theme;
pub use workspace::Workspace;
