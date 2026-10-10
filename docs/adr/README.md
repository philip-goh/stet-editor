# Architecture decision records

One file per decision. ADRs are immutable once accepted: to change a
decision, add a new ADR that supersedes the old one and update both files'
status headers.

## Template

Copy this into `NNNN-short-title.md` with the next number:

```markdown
# NNNN. Title

- Status: Proposed | Accepted | Superseded by ADR-XXXX
- Date: YYYY-MM-DD

## Context

What forces are at play? What problem are we solving? Include the concrete
constraints (platforms, performance targets, team size).

## Decision

What we will do, stated in the active voice. Include enough specifics that a
new contributor can follow it without asking.

## Consequences

What becomes easier, what becomes harder, and what we are knowingly
deferring. Include the migration/exit path for decisions that could be
revisited.

## Alternatives considered

Each alternative, and why it was not chosen.
```

## Index

| ADR | Decision |
| --- | --- |
| [0001](0001-record-architecture-decisions.md) | Record architecture decisions |
| [0002](0002-editor-core-is-ui-agnostic.md) | Keep the editor core UI-agnostic |
| [0003](0003-gui-framework-gpui.md) | Build the UI on GPUI, not Floem |
| [0004](0004-text-buffer-rope.md) | Rope-backed text buffer with `ropey` |
| [0005](0005-syntax-highlighting.md) | Tree-sitter for syntax highlighting |
| [0006](0006-plugin-system.md) | WASM-first, capability-based plugins |
| [0007](0007-vscode-extension-compatibility.md) | VS Code extension compatibility strategy |
