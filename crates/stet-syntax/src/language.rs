//! Mapping between file paths and tree-sitter grammars.

use std::path::Path;
use std::sync::OnceLock;

use tree_sitter_highlight::HighlightConfiguration;

/// A language the editor can highlight.
///
/// Grammars are compiled into the binary. Adding a language means adding a
/// variant here, a grammar crate in `Cargo.toml`, and an arm in
/// [`Language::config`]. See `docs/development.md`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Language {
    /// No highlighting.
    PlainText,
    /// Rust.
    Rust,
    /// JavaScript (including `.mjs`, `.cjs` and `.jsx` files).
    JavaScript,
    /// JSON (including JSON Lines).
    Json,
    /// TOML.
    Toml,
}

impl Language {
    /// Detects a language from a file path's extension.
    pub fn from_path(path: &Path) -> Self {
        path.extension()
            .and_then(|extension| extension.to_str())
            .map_or(Self::PlainText, Self::from_extension)
    }

    /// Detects a language from a bare extension, without the leading dot.
    pub fn from_extension(extension: &str) -> Self {
        match extension.to_ascii_lowercase().as_str() {
            "rs" => Self::Rust,
            "js" | "mjs" | "cjs" | "jsx" => Self::JavaScript,
            "json" | "jsonl" | "ndjson" => Self::Json,
            "toml" => Self::Toml,
            _ => Self::PlainText,
        }
    }

    /// Returns a human-readable name for the status bar.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::PlainText => "Plain Text",
            Self::Rust => "Rust",
            Self::JavaScript => "JavaScript",
            Self::Json => "JSON",
            Self::Toml => "TOML",
        }
    }

    /// Returns `true` if the language has no grammar attached.
    pub fn is_plain(self) -> bool {
        self == Self::PlainText
    }

    /// Returns the shared highlight configuration for this language, if it
    /// has one.
    ///
    /// Configurations compile tree-sitter queries, so they are built once
    /// per language and cached for the lifetime of the process.
    pub(crate) fn config(self) -> Option<&'static HighlightConfiguration> {
        fn build(
            slot: &'static OnceLock<Option<HighlightConfiguration>>,
            language: tree_sitter::Language,
            name: &str,
            highlights: &str,
            injections: &str,
        ) -> Option<&'static HighlightConfiguration> {
            slot.get_or_init(|| {
                HighlightConfiguration::new(language, name, highlights, injections, "")
                    .map(|mut config| {
                        config.configure(crate::highlight::HIGHLIGHT_NAMES);
                        config
                    })
                    .ok()
            })
            .as_ref()
        }

        match self {
            Self::PlainText => None,
            Self::Rust => {
                static CONFIG: OnceLock<Option<HighlightConfiguration>> = OnceLock::new();
                build(
                    &CONFIG,
                    tree_sitter_rust::LANGUAGE.into(),
                    "rust",
                    tree_sitter_rust::HIGHLIGHTS_QUERY,
                    tree_sitter_rust::INJECTIONS_QUERY,
                )
            }
            Self::JavaScript => {
                static CONFIG: OnceLock<Option<HighlightConfiguration>> = OnceLock::new();
                build(
                    &CONFIG,
                    tree_sitter_javascript::LANGUAGE.into(),
                    "javascript",
                    tree_sitter_javascript::HIGHLIGHT_QUERY,
                    tree_sitter_javascript::INJECTIONS_QUERY,
                )
            }
            Self::Json => {
                static CONFIG: OnceLock<Option<HighlightConfiguration>> = OnceLock::new();
                build(
                    &CONFIG,
                    tree_sitter_json::LANGUAGE.into(),
                    "json",
                    tree_sitter_json::HIGHLIGHTS_QUERY,
                    "",
                )
            }
            Self::Toml => {
                static CONFIG: OnceLock<Option<HighlightConfiguration>> = OnceLock::new();
                build(
                    &CONFIG,
                    tree_sitter_toml_ng::LANGUAGE.into(),
                    "toml",
                    tree_sitter_toml_ng::HIGHLIGHTS_QUERY,
                    "",
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_languages_from_extensions() {
        assert_eq!(Language::from_extension("rs"), Language::Rust);
        assert_eq!(Language::from_extension("RS"), Language::Rust);
        assert_eq!(Language::from_extension("jsx"), Language::JavaScript);
        assert_eq!(Language::from_extension("jsonl"), Language::Json);
        assert_eq!(Language::from_extension("toml"), Language::Toml);
        assert_eq!(Language::from_extension("md"), Language::PlainText);
    }

    #[test]
    fn detects_languages_from_paths() {
        assert_eq!(
            Language::from_path(Path::new("/tmp/main.rs")),
            Language::Rust
        );
        assert_eq!(
            Language::from_path(Path::new("/tmp/no-extension")),
            Language::PlainText
        );
    }
}
