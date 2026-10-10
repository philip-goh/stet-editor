//! Plugin manifests: what a plugin declares before it is ever executed.

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

/// The metadata file every plugin ships, in `stet-plugin.json`.
///
/// The shape intentionally mirrors the useful half of a VS Code
/// `package.json` (`engines`, `activationEvents`, `contributes`) so the
/// future VS Code bridge (#[see ADR 0007]) is a translation, not a
/// redesign.
///
/// [ADR 0007]: ../../docs/adr/0007-vscode-extension-compatibility.md
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
    /// Unique plugin identifier, e.g. `stet.rust-analyzer`.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Plugin version, in semver.
    pub version: Version,
    /// Short description shown in the plugin browser.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Plugin authors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    /// Which editor versions the plugin supports.
    pub engines: EngineConstraint,
    /// Events that cause the plugin to be loaded.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub activation: Vec<ActivationEvent>,
    /// Static contributions to the editor UI.
    #[serde(default)]
    pub contributes: Contributions,
}

/// The editor-version constraint a plugin declares.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineConstraint {
    /// A semver requirement such as `^0.1` or `>=0.1, <0.3`.
    pub stet: String,
}

/// An event that activates a plugin.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum ActivationEvent {
    /// Activate when the editor starts.
    Startup,
    /// Activate when a document of the given language is opened.
    OnLanguage {
        /// Language identifier, e.g. `rust`.
        language: String,
    },
    /// Activate when the given command is invoked.
    OnCommand {
        /// Command identifier as listed in [`CommandContribution`].
        command: String,
    },
}

/// Static contributions a plugin makes to the editor.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contributions {
    /// Commands the plugin adds to the command palette.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<CommandContribution>,
    /// Languages the plugin teaches the editor about.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub languages: Vec<LanguageContribution>,
}

/// A command a plugin wants to appear in the command palette.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandContribution {
    /// Unique command identifier, e.g. `rust-analyzer.reload`.
    pub id: String,
    /// Palette title, e.g. `Rust Analyzer: Reload`.
    pub title: String,
}

/// A language contribution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageContribution {
    /// Language identifier, e.g. `rust`.
    pub id: String,
    /// File extensions without the leading dot.
    pub extensions: Vec<String>,
}

/// Errors produced while loading a manifest.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    /// The manifest is not valid JSON or is missing fields.
    #[error("invalid plugin manifest: {0}")]
    Json(#[from] serde_json::Error),
    /// The plugin identifier does not follow the required format.
    #[error("invalid plugin id `{0}`: expected `<publisher>.<name>` using [a-z0-9-]")]
    InvalidId(String),
    /// The engine constraint is not a valid semver requirement.
    #[error("invalid engine constraint `{0}`: {1}")]
    InvalidEngineRequirement(String, semver::Error),
}

impl PluginManifest {
    /// Parses and validates a manifest from JSON.
    pub fn from_json(json: &str) -> Result<Self, ManifestError> {
        let manifest: Self = serde_json::from_str(json)?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Validates the manifest beyond JSON structure.
    ///
    /// Ensures the id is `<publisher>.<name>`, the engine requirement
    /// parses, and contribution ids are unique.
    pub fn validate(&self) -> Result<(), ManifestError> {
        if !is_valid_plugin_id(&self.id) {
            return Err(ManifestError::InvalidId(self.id.clone()));
        }
        if let Err(error) = VersionReq::parse(&self.engines.stet) {
            return Err(ManifestError::InvalidEngineRequirement(
                self.engines.stet.clone(),
                error,
            ));
        }

        let mut command_ids = std::collections::HashSet::new();
        for command in &self.contributes.commands {
            if !command_ids.insert(command.id.as_str()) || !is_valid_plugin_id(&command.id) {
                return Err(ManifestError::InvalidId(command.id.clone()));
            }
        }
        Ok(())
    }
}

/// Returns `true` if `id` looks like `publisher.name`, both made of ASCII
/// lowercase letters, digits and hyphens, separated by exactly one dot.
fn is_valid_plugin_id(id: &str) -> bool {
    let mut parts = id.split('.');
    let (Some(publisher), Some(name), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    let valid_part = |part: &str| {
        !part.is_empty()
            && part
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    };
    valid_part(publisher) && valid_part(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"
    {
        "id": "stet.rust-analyzer",
        "name": "Rust Analyzer",
        "version": "0.3.1",
        "description": "Language support for Rust",
        "authors": ["Philip Goh"],
        "engines": { "stet": "^0.1" },
        "activation": [
            { "onLanguage": { "language": "rust" } },
            { "onCommand": { "command": "rust-analyzer.reload" } },
            "startup"
        ],
        "contributes": {
            "commands": [
                { "id": "rust-analyzer.reload", "title": "Rust Analyzer: Reload" }
            ],
            "languages": [
                { "id": "rust", "extensions": ["rs"] }
            ]
        }
    }
    "#;

    #[test]
    fn parses_a_valid_manifest() {
        let manifest = PluginManifest::from_json(VALID).unwrap();
        assert_eq!(manifest.id, "stet.rust-analyzer");
        assert_eq!(manifest.version, Version::new(0, 3, 1));
        assert_eq!(manifest.activation.len(), 3);
        assert_eq!(manifest.contributes.commands.len(), 1);
        assert_eq!(manifest.contributes.languages[0].extensions, ["rs"]);
    }

    #[test]
    fn rejects_invalid_plugin_ids() {
        let json = VALID.replace("stet.rust-analyzer", "Not A Plugin Id");
        assert!(matches!(
            PluginManifest::from_json(&json),
            Err(ManifestError::InvalidId(_))
        ));
    }

    #[test]
    fn rejects_invalid_engine_requirements() {
        let json = VALID.replace("\"^0.1\"", "\"not-a-requirement\"");
        assert!(matches!(
            PluginManifest::from_json(&json),
            Err(ManifestError::InvalidEngineRequirement(_, _))
        ));
    }

    #[test]
    fn rejects_unknown_fields() {
        let json = VALID.replace("\"name\"", "\"nayme\"");
        assert!(matches!(
            PluginManifest::from_json(&json),
            Err(ManifestError::Json(_))
        ));
    }

    #[test]
    fn rejects_duplicate_command_ids() {
        let json = VALID.replace(
            r#"{ "id": "rust-analyzer.reload", "title": "Rust Analyzer: Reload" }"#,
            r#"{ "id": "rust-analyzer.reload", "title": "One" },
               { "id": "rust-analyzer.reload", "title": "Two" }"#,
        );
        assert!(matches!(
            PluginManifest::from_json(&json),
            Err(ManifestError::InvalidId(_))
        ));
    }
}
