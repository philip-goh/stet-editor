//! Plugin ABI and manifest types for the Stet editor.
//!
//! The plugin strategy is documented in
//! `docs/adr/0006-plugin-system.md` and
//! `docs/adr/0007-vscode-extension-compatibility.md`. The short version:
//!
//! 1. Plugins are sandboxed WebAssembly modules with an explicit,
//!    capability-based host interface.
//! 2. This crate defines the data types crossing that boundary
//!    ([`PluginManifest`], [`CommandSpec`], ...) and the Rust-side traits
//!    ([`Host`], [`Plugin`]) that a future `stet-plugin-host` crate will
//!    implement on top of a WASM runtime.
//! 3. A later compatibility layer will translate VS Code extension
//!    manifests and API calls into these same primitives, so the rest of
//!    the editor never knows where a plugin came from.
//!
//! Nothing in this crate executes third-party code yet; it is deliberately
//! dependency-light so the ABI can be reviewed and tested in isolation.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod api;
mod manifest;

pub use api::{CommandSpec, Host, MessageLevel, Plugin, PluginError, PluginInfo, PluginResult};
pub use manifest::{
    ActivationEvent, CommandContribution, Contributions, EngineConstraint, LanguageContribution,
    ManifestError, PluginManifest,
};
