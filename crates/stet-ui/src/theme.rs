//! The editor color theme.

use gpui::{Hsla, rgb, rgba};

/// Converts an RGB hex color into the [Hsla] space used by themes.
fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

/// Converts an RGBA hex color into the [Hsla] space used by themes.
fn ca(hex: u32) -> Hsla {
    rgba(hex).into()
}
use stet_syntax::HighlightKind;

/// Syntax highlighting colors.
#[derive(Clone, Debug)]
pub struct SyntaxTheme {
    /// Attributes and annotations.
    pub attribute: Hsla,
    /// Built-in constants.
    pub constant: Hsla,
    /// Comments.
    pub comment: Hsla,
    /// Function and method names.
    pub function: Hsla,
    /// Keywords.
    pub keyword: Hsla,
    /// Labels and lifetimes.
    pub label: Hsla,
    /// Numeric literals.
    pub number: Hsla,
    /// Operators.
    pub operator: Hsla,
    /// Properties and keys.
    pub property: Hsla,
    /// Punctuation.
    pub punctuation: Hsla,
    /// String literals.
    pub string: Hsla,
    /// Tags in markup.
    pub tag: Hsla,
    /// Types.
    pub r#type: Hsla,
    /// Variables and identifiers.
    pub variable: Hsla,
    /// Built-in variables.
    pub builtin: Hsla,
    /// Function parameters.
    pub parameter: Hsla,
}

impl SyntaxTheme {
    /// Returns the color for a highlight kind.
    pub fn color(&self, kind: HighlightKind) -> Hsla {
        match kind {
            HighlightKind::Attribute => self.attribute,
            HighlightKind::Constant => self.constant,
            HighlightKind::Comment => self.comment,
            HighlightKind::Function => self.function,
            HighlightKind::Keyword => self.keyword,
            HighlightKind::Label => self.label,
            HighlightKind::Number => self.number,
            HighlightKind::Operator => self.operator,
            HighlightKind::Property => self.property,
            HighlightKind::Punctuation => self.punctuation,
            HighlightKind::String => self.string,
            HighlightKind::Tag => self.tag,
            HighlightKind::Type => self.r#type,
            HighlightKind::Variable => self.variable,
            HighlightKind::Builtin => self.builtin,
            HighlightKind::Parameter => self.parameter,
        }
    }
}

/// The editor's color theme.
///
/// A single dark theme ships with v0.1; theme loading and switching are on
/// the roadmap. All colors live here so a theme system has one obvious
/// place to grow.
#[derive(Clone, Debug)]
pub struct Theme {
    /// Editor and window background.
    pub background: Hsla,
    /// Chrome surfaces such as the tab strip and status bar.
    pub surface: Hsla,
    /// Raised surfaces such as the active tab and palette.
    pub surface_raised: Hsla,
    /// Primary text color.
    pub foreground: Hsla,
    /// Secondary text color.
    pub muted: Hsla,
    /// Accent color used for selection and focus.
    pub accent: Hsla,
    /// Borders and separators.
    pub border: Hsla,
    /// Selection highlight.
    pub selection: Hsla,
    /// Find-match highlight.
    pub search_match: Hsla,
    /// Highlight for the match the find bar is currently on.
    pub search_current: Hsla,
    /// Caret color.
    pub caret: Hsla,
    /// Line number color.
    pub gutter: Hsla,
    /// Line number color for the cursor's line.
    pub gutter_active: Hsla,
    /// Syntax colors.
    pub syntax: SyntaxTheme,
}

impl Theme {
    /// Returns the default dark theme.
    pub fn dark() -> Self {
        Self {
            background: c(0x1b1e24),
            surface: c(0x22262e),
            surface_raised: c(0x2a2f39),
            foreground: c(0xd6dae0),
            muted: c(0x8a919c),
            accent: c(0x4c8dff),
            border: c(0x343a45),
            selection: ca(0x3d64c880),
            search_match: ca(0x4c8dff40),
            search_current: ca(0xf0a44c66),
            caret: c(0xe6e9ee),
            gutter: c(0x5c6470),
            gutter_active: c(0xaab2bf),
            syntax: SyntaxTheme {
                attribute: c(0xd19a66),
                constant: c(0xd19a66),
                comment: c(0x7f848e),
                function: c(0x61afef),
                keyword: c(0xc678dd),
                label: c(0xe06c75),
                number: c(0xd19a66),
                operator: c(0x56b6c2),
                property: c(0xe06c75),
                punctuation: c(0xabb2bf),
                string: c(0x98c379),
                tag: c(0xe06c75),
                r#type: c(0xe5c07b),
                variable: c(0xabb2bf),
                builtin: c(0x56b6c2),
                parameter: c(0xd19a66),
            },
        }
    }
}
