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

- [ ] IME and composition via `EntityInputHandler` (CJK, dead keys)
- [ ] Word-wise selection (`ctrl/alt+shift+arrow`) and double-click select
- [ ] Multi-cursor basics: add cursor above/below, select next occurrence
- [ ] Line wrapping, with a "wrap" toggle
- [ ] Find/replace with incremental search
- [ ] Indentation: auto-indent on newline, tab/shift-tab block indent
- [ ] Bracket matching and auto-closing pairs
- [ ] Deletion grouping in undo (word/line deletes as one step)
- [ ] Content-hash dirty tracking so undo-to-saved shows clean

## M2 — Productivity

Goal: competitive with a lightly used VS Code for everyday files.

- [ ] File tree / open folder with fuzzy file open
- [ ] Split panes and tab drag/ordering
- [ ] Settings system (`settings.toml`) with live reload
- [ ] Theme system with light theme and theme files
- [ ] Recent files and session restore
- [ ] Git gutter (line-level diff against HEAD)
- [ ] Search in files
- [ ] Background + incremental highlighting for large files

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
