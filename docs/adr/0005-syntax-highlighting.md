# 0005. Tree-sitter for syntax highlighting

- Status: Accepted
- Date: 2026-10-10

## Context

Syntax highlighting shapes the editor's feel more than almost anything
else, and it is also a performance hot spot. The realistic options:

- **TextMate grammars** (what VS Code uses, via `syntect` in Rust):
  battle-tested, thousands of grammars exist, but line-based regex
  highlighting has no structural understanding, is slow on large files, and
  gives no parse tree we can build features on (folding, smart selection,
  injections).
- **Tree-sitter**: incremental, error-tolerant parsers with query-based
  highlighting. Larger binaries (C grammars), and the query/capture model
  needs curation, but it is the foundation modern editors are converging on.

The editor also wants a parse tree eventually for structural features, and
the plugin story (LSP, semantic tokens) aligns with tree-sitter's model.

## Decision

Use `tree-sitter` + `tree-sitter-highlight` with grammars compiled into
the binary. `stet-syntax` owns one `Highlighter` per document and exposes a
coarse, editor-owned vocabulary of `HighlightKind`s (keyword, function,
string, ...) rather than exposing raw tree-sitter capture names. Themes map
kinds to colors, so grammar capture churn cannot break themes.

Highlighting happens synchronously on text change and produces a per-line
span cache; documents over `MAX_HIGHLIGHT_BYTES` (1 MiB) are not highlighted
until background highlighting lands (roadmap M2).

Initial languages: Rust, JavaScript, JSON, TOML. Adding a language is a
documented, mechanical change (see docs/development.md).

## Consequences

- Parse trees enable future structural features without a rewrite.
- Binary size grows with each grammar (~hundreds of KB each); we add
  languages deliberately.
- The initial implementation re-parses on every change because
  `tree-sitter-highlight`'s high-level API does not accept an existing tree.
  `EditSummary` is already returned by `stet-core` so incremental parsing or
  background parsing can be adopted without changing the core API.
- Grammar crates bring C code; the workspace `unsafe` ban still holds
  because the C lives behind the grammar crates' safe APIs.

## Alternatives considered

- **syntect / TextMate**: huge grammar ecosystem and it is what VS Code
  compat will eventually need for theme compatibility, but no parse tree,
  weaker performance, and no path to structural features. We may still use
  TextMate scopes when translating VS Code themes in ADR-0007 territory.
- **Semantic highlighting via LSP only**: too slow, not available without a
  language server, and does not cover every file we open.
- **Hand-written lexers**: not maintainable across languages.
