//! Tree-sitter based highlighting with a per-line span cache.

use std::ops::Range;

use tree_sitter_highlight::{
    Highlight, HighlightConfiguration, HighlightEvent, Highlighter as TreeSitterHighlighter,
};

use crate::language::Language;

/// Documents larger than this many bytes are not highlighted.
///
/// Tree-sitter querying is fast, but a full parse per keystroke is not free
/// on very large files. Background and incremental highlighting are roadmap
/// items; until then the editor stays responsive by turning highlighting off
/// for genuinely huge buffers.
pub const MAX_HIGHLIGHT_BYTES: usize = 1024 * 1024;

/// The capture names the editor understands, in the order they are
/// configured with tree-sitter.
///
/// The order defines the [`Highlight`] indices tree-sitter hands back, so
/// [`kind_for`] maps them by name and stays correct if the list changes.
pub(crate) const HIGHLIGHT_NAMES: &[&str] = &[
    "attribute",
    "boolean",
    "comment",
    "constant",
    "constructor",
    "function",
    "function.builtin",
    "keyword",
    "label",
    "number",
    "operator",
    "property",
    "punctuation",
    "string",
    "tag",
    "type",
    "variable",
    "variable.builtin",
    "variable.parameter",
];

/// A syntax class the theme can assign a color to.
///
/// This is intentionally a small, editor-owned vocabulary rather than the
/// full tree-sitter capture space: themes stay stable even when grammars
/// gain new capture names.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum HighlightKind {
    /// Attributes and annotations, e.g. `#[derive]`.
    Attribute,
    /// Built-in constants, e.g. `true` and `false`.
    Constant,
    /// Comments.
    Comment,
    /// Function and method names.
    Function,
    /// Keywords, e.g. `fn`, `let`, `if`.
    Keyword,
    /// Labels and lifetimes.
    Label,
    /// Numeric literals.
    Number,
    /// Operators.
    Operator,
    /// Properties and keys, e.g. JSON object keys.
    Property,
    /// Punctuation such as braces and commas.
    Punctuation,
    /// String literals.
    String,
    /// Tags in markup languages.
    Tag,
    /// Types and type-like names.
    Type,
    /// Variables and other identifiers.
    #[default]
    Variable,
    /// Built-in variables, e.g. `self`.
    Builtin,
    /// Function parameters.
    Parameter,
}

fn kind_for(highlight: Highlight) -> HighlightKind {
    match HIGHLIGHT_NAMES.get(highlight.0) {
        Some(&"attribute") => HighlightKind::Attribute,
        Some(&"boolean" | &"constant") => HighlightKind::Constant,
        Some(&"comment") => HighlightKind::Comment,
        Some(&"constructor" | &"function" | &"function.builtin") => HighlightKind::Function,
        Some(&"keyword") => HighlightKind::Keyword,
        Some(&"label") => HighlightKind::Label,
        Some(&"number") => HighlightKind::Number,
        Some(&"operator") => HighlightKind::Operator,
        Some(&"property") => HighlightKind::Property,
        Some(&"punctuation") => HighlightKind::Punctuation,
        Some(&"string") => HighlightKind::String,
        Some(&"tag") => HighlightKind::Tag,
        Some(&"type") => HighlightKind::Type,
        Some(&"variable.builtin") => HighlightKind::Builtin,
        Some(&"variable.parameter") => HighlightKind::Parameter,
        _ => HighlightKind::Variable,
    }
}

/// A highlighted byte range within a single line.
///
/// `start` and `end` are byte offsets relative to the start of the line and
/// never include the line break. Ranges are sorted, non-overlapping and
/// bounded by the line's content length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HighlightSpan {
    /// Start byte offset within the line.
    pub start: usize,
    /// End byte offset within the line, exclusive.
    pub end: usize,
    /// The syntax class to style the range with.
    pub kind: HighlightKind,
}

impl HighlightSpan {
    /// Returns the span as a range.
    pub fn range(&self) -> Range<usize> {
        self.start..self.end
    }
}

fn no_injection<'a>(_name: &str) -> Option<&'a HighlightConfiguration> {
    None
}

/// Owns the parse/highlight state for one document.
///
/// Call [`Highlighter::refresh`] with the full document text whenever the
/// text changes. Queries are answered from a per-line cache afterwards, so
/// painting a frame never re-parses.
pub struct Highlighter {
    language: Language,
    engine: TreeSitterHighlighter,
    active: bool,
    lines: Vec<Vec<HighlightSpan>>,
}

impl std::fmt::Debug for Highlighter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Highlighter")
            .field("language", &self.language)
            .field("active", &self.active)
            .field("lines", &self.lines.len())
            .finish()
    }
}

impl Highlighter {
    /// Creates a highlighter for `language` with no text parsed yet.
    pub fn new(language: Language) -> Self {
        Self {
            language,
            engine: TreeSitterHighlighter::new(),
            active: false,
            lines: Vec::new(),
        }
    }

    /// Returns the current language.
    pub fn language(&self) -> Language {
        self.language
    }

    /// Switches to another language, clearing cached spans.
    pub fn set_language(&mut self, language: Language) {
        if self.language != language {
            self.language = language;
            self.active = false;
            self.lines.clear();
        }
    }

    /// Reparses `text` and rebuilds the per-line span cache.
    ///
    /// Languages without a grammar and documents larger than
    /// [`MAX_HIGHLIGHT_BYTES`] simply clear the cache.
    pub fn refresh(&mut self, text: &str) {
        self.active = false;
        self.lines.clear();

        let Some(config) = self.language.config() else {
            return;
        };
        if text.len() > MAX_HIGHLIGHT_BYTES {
            return;
        }

        let Ok(events) = self
            .engine
            .highlight(config, text.as_bytes(), None, None, no_injection)
        else {
            return;
        };

        let mut stack: Vec<Highlight> = Vec::new();
        let mut spans: Vec<(Range<usize>, Highlight)> = Vec::new();
        for event in events {
            let Ok(event) = event else { return };
            match event {
                HighlightEvent::HighlightStart(highlight) => stack.push(highlight),
                HighlightEvent::HighlightEnd => {
                    stack.pop();
                }
                HighlightEvent::Source { start, end } => {
                    if let Some(&highlight) = stack.last()
                        && start < end
                    {
                        spans.push((start..end, highlight));
                    }
                }
            }
        }

        self.lines = bucket_spans_by_line(text, &spans);
        self.active = true;
    }

    /// Returns the highlighted spans of `line`, or an empty slice if the
    /// line is out of bounds or the highlighter is inactive.
    pub fn line_spans(&self, line: usize) -> &[HighlightSpan] {
        self.lines.get(line).map_or(&[], Vec::as_slice)
    }

    /// Returns `true` if the cache holds spans from a successful parse.
    pub fn is_active(&self) -> bool {
        self.active
    }
}

/// Splits absolute byte spans into per-line, line-relative, clamped and
/// non-overlapping spans.
fn bucket_spans_by_line(
    text: &str,
    spans: &[(Range<usize>, Highlight)],
) -> Vec<Vec<HighlightSpan>> {
    let bytes = text.as_bytes();

    let mut line_starts = vec![0usize];
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            line_starts.push(index + 1);
        }
    }

    let mut lines: Vec<Vec<HighlightSpan>> = vec![Vec::new(); line_starts.len()];

    for (range, highlight) in spans {
        let kind = kind_for(*highlight);
        let start_line = match line_starts.binary_search(&range.start) {
            Ok(line) => line,
            Err(index) => index.saturating_sub(1),
        };
        let mut line = start_line;
        while line < line_starts.len() && line_starts[line] < range.end {
            let line_start = line_starts[line];
            let line_end = line_starts.get(line + 1).copied().unwrap_or(bytes.len());

            // Exclude the line break from highlightable content.
            let mut content_end = line_end;
            if content_end > line_start && bytes[content_end - 1] == b'\n' {
                content_end -= 1;
                if content_end > line_start && bytes[content_end - 1] == b'\r' {
                    content_end -= 1;
                }
            }

            let start = range.start.max(line_start);
            let end = range.end.min(content_end);
            if start < end {
                let span = HighlightSpan {
                    start: start - line_start,
                    end: end - line_start,
                    kind,
                };
                // Keep spans sorted and non-overlapping: later captures
                // should not retrace bytes an earlier one styled.
                let trimmed_start = match lines[line].last() {
                    Some(last) if last.end > span.start => last.end,
                    _ => span.start,
                };
                if trimmed_start < span.end {
                    lines[line].push(HighlightSpan {
                        start: trimmed_start,
                        ..span
                    });
                }
            }
            line += 1;
        }
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_rust_keywords() {
        let mut highlighter = Highlighter::new(Language::Rust);
        highlighter.refresh("fn main() {}\n");
        assert!(highlighter.is_active());

        let spans = highlighter.line_spans(0);
        let keyword = spans
            .iter()
            .find(|span| span.kind == HighlightKind::Keyword);
        let keyword = keyword.expect("expected a keyword span for `fn`");
        assert_eq!(&"fn main() {}"[keyword.range()], "fn");
    }

    #[test]
    fn highlights_json_literals() {
        let mut highlighter = Highlighter::new(Language::Json);
        highlighter.refresh("{\"answer\": 42}\n");
        let spans = highlighter.line_spans(0);
        assert!(spans.iter().any(|span| span.kind == HighlightKind::String));
        assert!(spans.iter().any(|span| span.kind == HighlightKind::Number));
    }

    #[test]
    fn plain_text_has_no_spans() {
        let mut highlighter = Highlighter::new(Language::PlainText);
        highlighter.refresh("fn main() {}\n");
        assert!(!highlighter.is_active());
        assert!(highlighter.line_spans(0).is_empty());
    }

    #[test]
    fn spans_are_clamped_to_line_content() {
        let mut highlighter = Highlighter::new(Language::Rust);
        highlighter.refresh("// a comment\nfn x() {}\n");
        for (line, content) in ["// a comment", "fn x() {}"].iter().enumerate() {
            for span in highlighter.line_spans(line) {
                assert!(span.start < span.end, "empty span on line {line}");
                assert!(
                    span.end <= content.len(),
                    "span {:?} exceeds content length on line {line}",
                    span
                );
            }
        }
    }

    #[test]
    fn out_of_bounds_lines_are_empty() {
        let mut highlighter = Highlighter::new(Language::Rust);
        highlighter.refresh("fn main() {}\n");
        assert!(highlighter.line_spans(99).is_empty());
    }

    #[test]
    fn refreshing_clears_previous_spans() {
        let mut highlighter = Highlighter::new(Language::Rust);
        highlighter.refresh("fn main() {}\n");
        assert!(!highlighter.line_spans(0).is_empty());
        highlighter.refresh("");
        assert!(highlighter.line_spans(0).is_empty());
        assert!(highlighter.is_active());
    }

    #[test]
    fn switching_language_clears_cache() {
        let mut highlighter = Highlighter::new(Language::Rust);
        highlighter.refresh("fn main() {}\n");
        highlighter.set_language(Language::PlainText);
        assert!(!highlighter.is_active());
        assert!(highlighter.line_spans(0).is_empty());
    }
}
