# AGENTS.md

Orientation for AI coding agents (and humans in a hurry). Read this before
changing code. Deeper detail lives in [docs/architecture.md](docs/architecture.md)
and the [ADRs](docs/adr/).

## What this is

Stet is a cross-platform text editor in Rust, UI built on GPUI. The goal is
a Sublime-like weight with VS Code-like capability, plus a plugin system
that can eventually run translated VS Code extensions.

## Golden rules

1. **Dependency direction is one-way.** `apps/stet -> stet-ui -> {stet-core,
   stet-syntax, stet-plugin }`, and `stet-syntax -> stet-core`. `stet-core`
   must never depend on GPUI or any UI concern. If you need UI data in core,
   design a plain-data API instead (see [ADR 0002](docs/adr/0002-editor-core-is-ui-agnostic.md)).
2. **All offsets in `stet-core` are byte offsets** unless a name says
   otherwise (`len_chars`, `Position::column`). Don't mix units; that is how
   editor bugs happen.
3. **Commands have exactly one implementation.** Every user-visible command
   goes through an action handler in `stet-ui/src/workspace.rs`. Keyboard
   shortcuts, toolbar buttons and palette entries all call the same
   `perform_*` method. Never wire a button to bespoke logic.
4. **`unsafe` is forbidden** in workspace crates (workspace lints). C comes
   in only through tree-sitter grammar crates.
5. **No comments unless they explain *why*.** Code should read like the
   docs are nearby; doc comments for public items are mandatory
   (`missing_docs` is a warning, and CI denies warnings).
6. **Tests are part of the change.** Logic changes to `stet-core` (or
   manifest/highlight logic) need unit tests. UI changes need at least a
   compile + smoke run described in the PR.

## Commands

```sh
just              # list all recipes
just fmt          # cargo fmt --all
just lint         # clippy with -D warnings
just test         # cargo test --workspace
just run          # launch the editor
just deny         # license/advisory/source policy
just ci           # fmt-check + lint + test (what CI runs)
```

Prefer `cargo test -p stet-core` while iterating on the engine; the GPUI
crate is slow to link.

## Repo map

```
apps/stet          binary: arg parsing + stet_ui::launch
crates/stet-core   buffer/text engine (no UI), all unit-tested
  src/text.rs      TextBuffer: rope, positions, graphemes, words, edits
  src/document.rs  Document: path, dirty state, atomic save, history
  src/history.rs   undo/redo with typing coalescing
  src/selection.rs anchor/head byte selection
  src/edit.rs      AppliedEdit + tree-sitter EditSummary
crates/stet-syntax Language enum + Highlighter (tree-sitter, per-line cache)
crates/stet-plugin PluginManifest + Host/Plugin traits (WASM ABI contracts)
crates/stet-ui     GPUI layer
  src/app.rs       launch(), key bindings
  src/workspace.rs tabs, toolbar, status bar, palette, action handlers
  src/editor.rs    EditorView state + LineElement custom element
  src/palette.rs   palette state and filtering
  src/theme.rs     colors
  src/actions.rs   actions! vocabulary
docs/adr           decision records (one file per decision)
```

## How to add things

### A command

1. Add the action to `crates/stet-ui/src/actions.rs` (doc comment required).
2. Implement `perform_<name>(&mut self, window, cx)` in `Workspace`.
3. Add a thin `on_<name>` handler and register it with `.on_action(...)` in
   `Workspace::render`.
4. Add a `PaletteCommand` variant + title if it belongs in the palette.
5. Bind keys in `stet-ui/src/app.rs`.

### A language

1. Add the grammar crate to `Cargo.toml` workspace deps and
   `crates/stet-syntax/Cargo.toml`.
2. Add a `Language` variant, extension mapping, and `config()` arm in
   `crates/stet-syntax/src/language.rs`.
3. Add a test in `crates/stet-syntax/src/highlight.rs`.
4. Update the README feature list.

### A settings/architecture decision

Write an ADR in `docs/adr/`, next number, using the template in
`docs/adr/README.md`. Do not edit accepted ADRs; supersede them.

## Gotchas

- **GPUI version pin.** We pin `gpui = "0.2.2"`. GPUI's API moves fast; do
  not bump without running the app and checking `editor.rs`'s `Element`
  impl against the new signatures.
- **Runtime shaders.** `stet-ui` enables `gpui/runtime_shaders` by default
  because many machines lack full Xcode. Don't remove it without reading
  `crates/stet-ui/Cargo.toml`.
- **`uniform_list` ids.** Element ids are resolved per window; lines use
  `ElementId::named_usize("line", ..)`-style ids. Keep them unique.
- **IME is not implemented.** Typing uses `KeyEvent::key_char`; dead keys
  and CJK composition need a real `EntityInputHandler` (roadmap M1).
- **Palette routes actions.** When the palette is open, editing actions are
  intercepted by the workspace handlers and must not reach the editor.

## Verification checklist before finishing

```sh
just ci        # fmt-check, clippy -D warnings, tests
just deny      # only needed when deps/lockfile change
just run       # smoke test on macOS when UI changed
```

Then update the roadmap checkboxes if you completed a roadmap item.
