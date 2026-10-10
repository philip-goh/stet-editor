//! The capability-based interface between plugins and the editor.
//!
//! These traits are the Rust mirror of the WASM host ABI. When the WASM
//! host lands, generated bindings will call through the same capability
//! surface, so plugin logic written against [`Host`] does not change.
//!
//! Design rules for this boundary:
//!
//! - Every host capability is explicit. There is no ambient access to the
//!   filesystem, network or editor internals.
//! - Plugins receive snapshots, never live references into editor state.
//! - The surface stays small; capabilities are added deliberately, with an
//!   ADR, as real plugins need them.

use semver::Version;

use crate::manifest::PluginManifest;

/// Result type used across the plugin boundary.
pub type PluginResult<T> = Result<T, PluginError>;

/// An error crossing the plugin/host boundary.
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    /// The plugin attempted something the host does not allow.
    #[error("capability denied: {0}")]
    Denied(String),
    /// The host rejected the request for another reason.
    #[error("{0}")]
    Host(String),
}

/// A command registration requested by a plugin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandSpec {
    /// Unique command identifier.
    pub id: String,
    /// Palette title.
    pub title: String,
}

/// Severity of a message shown to the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageLevel {
    /// Informational.
    Info,
    /// Warning.
    Warning,
    /// Error.
    Error,
}

/// The capabilities a plugin may use during activation.
///
/// Implemented by the editor for Rust-side plugins and by the WASM host for
/// sandboxed plugins. Plugins never get a richer handle than this.
pub trait Host {
    /// Registers a command that can be invoked from the palette.
    fn register_command(&mut self, command: CommandSpec) -> PluginResult<()>;

    /// Shows a message to the user.
    fn show_message(&mut self, level: MessageLevel, message: &str) -> PluginResult<()>;

    /// Returns a read-only snapshot of a document's text, by URI.
    ///
    /// Returns `None` if no open document matches. This is the only way a
    /// plugin reads editor state.
    fn document_text(&self, uri: &str) -> PluginResult<Option<String>>;
}

/// A plugin, in Rust form.
///
/// WASM plugins implement the same lifecycle through the host ABI; this
/// trait exists so the editor can host both without special-casing.
pub trait Plugin: Send {
    /// Called once when the plugin is activated.
    fn activate(&mut self, host: &mut dyn Host) -> PluginResult<()>;

    /// Called when the plugin is unloaded. Must not fail.
    fn deactivate(&mut self) {}
}

/// A loaded plugin and its metadata.
pub struct PluginInfo {
    /// The plugin's manifest.
    pub manifest: PluginManifest,
    /// The plugin API version the plugin was built against.
    pub api_version: Version,
}
