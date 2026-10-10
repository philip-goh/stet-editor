# Roadmap

Milestones are ordered; each one should leave the editor shippable. Check
items off as they land, and keep this file honest.

## M0 — Walking skeleton (current)

- [x] Workspace layout, CI, ADRs, agent docs
- [x] Rope-backed `TextBuffer` with positions, graphemes, words, CRLF
- [x] `Document` with atomic saves, dirty state, undo/redo
- [x] Tree-sitter highlighting for Rust, JavaScript, JSON, TOML
- [x] GPUI editor surface with virtualized lines, caret, selection
- [x] Tabs, command palette, status bar, native file dialogs
- [x] Plugin manifests and host ABI contracts
- [ ] Headless GPUI tests for workspace logic (`gpui::test-support`)
- [ ] Linux and Windows smoke testing

## M1 — Core editing

Goal: the editor is pleasant for a full day of Rust work.

Each iteration is a shippable slice with tests; check items off as they
land and keep this section in sync with reality.

### Iteration 1 — Editing polish

Feel-level fixes that build on the existing engine. All core changes are
unit-testable.

- [ ] Deletion grouping in undo: a backspace/delete run is one step; word
      and line kills are single steps (extend `History` coalescing)
- [ ] Content-hash dirty tracking so undo-to-saved shows clean
- [ ] Word-wise extend selection (`alt+shift+arrow`)
- [ ] Double-click selects word, triple-click selects line
- [ ] Indent/outdent: `Tab`/`Shift+Tab`, block indent for multi-line
      selections
- Acceptance: `stet-core` tests for coalescing, hashing and indent
  helpers; manual smoke of click/undo/indent flows.

### Iteration 2 — Text input and IME

The one gap that makes the editor unusable for CJK and dead-key input.

- [ ] `EntityInputHandler` + `ElementInputHandler` in the editor surface
- [ ] UTF-16 ↔ byte offset conversion helpers in `stet-core`, tested
- [ ] Preedit/marked text rendering (underline) and IME cursor area
- [ ] Auto-closing bracket/quote pairs that respect selections
- Acceptance: conversion tests; smoke test with a dead key and a CJK IME.

### Iteration 3 — Find and replace

- [ ] `stet-core` search model: literal matching, case sensitivity, match
      iteration over the rope, tested
- [ ] Find bar (`cmd+f`), next/previous (`enter`/`shift+enter`), match
      count, highlight matches in `LineElement`
- [ ] Replace one/all (`alt+cmd+f`), undo as one step for replace-all
- [ ] Palette entries and status messages for zero matches
- Acceptance: search unit tests; smoke test on a large file (no stalls
  while typing in the find field).

### Iteration 4 — Multi-cursor

- [ ] `SelectionSet` in core: normalized ranges, batch edit application
      with offset mapping, tested
- [ ] `cmd+d` select next occurrence, `cmd+alt+up/down` add cursor
- [ ] Render multiple carets/selections; escape collapses to primary
- Acceptance: core mapping tests; smoke test typing with three cursors and
  undoing the batch as one step.

### Iteration 5 — Highlighting at scale

- [ ] Incremental reparse using the `EditSummary` `stet-core` already
      returns
- [ ] Background highlighting task; UI paints cached spans and upgrades
      when the worker lands
- [ ] Remove the 1 MiB cap for well-behaved files; add a benchmark
- Acceptance: benchmark numbers in the PR; typing stays smooth in a
  multi-megabyte Rust file.

## M2 — Productivity

Goal: competitive with a lightly used VS Code for everyday files.

- [ ] File tree / open folder with fuzzy file open
- [ ] Line wrapping, with a "wrap" toggle
- [ ] Split panes and tab drag/ordering
- [ ] Settings system (`settings.toml`) with live reload
- [ ] Theme system with light theme and theme files
- [ ] Recent files and session restore
- [ ] Git gutter (line-level diff against HEAD)
- [ ] Search in files

## M3 — Plugin host

Goal: third parties can extend Stet without touching the repo.

- [ ] `stet-plugin-host` crate on `wasmtime` (or extism) implementing the
      `Host` trait across the WASM boundary (WIT-defined ABI)
- [ ] Plugin discovery/installation from a plugins directory and registry
- [ ] Plugin-contributed commands merged into the palette
- [ ] Capability prompts and a permission model
- [ ] LSP client as the first real capability (diagnostics, completion)

## M4 — VS Code extension compatibility

Goal: a meaningful subset of the VS Code ecosystem runs on Stet.

- [ ] Manifest translator (`.vsix` `package.json` → `PluginManifest`)
- [ ] `vscode` API shim over the WASM/JS boundary (text documents,
      workspace, commands, window messages)
- [ ] JS execution path for extensions (QuickJS or WASM-hosted runtime)
- [ ] Compatibility test suite against a curated set of popular extensions
- [ ] Document the unsupported-API story honestly

## M5 — Platform expansion

- [ ] First-class Linux packaging (deb/AppImage) and Windows installer
- [ ] Touch and trackpad gestures, on-screen keyboard
- [ ] Tablet/Android feasibility spike against GPUI's platform layer
