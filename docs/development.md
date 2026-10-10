# Development guide

## Prerequisites

- **Rust**: pinned by `rust-toolchain.toml`; `rustup` installs it
  automatically on first `cargo` command in the repo.
- **[just](https://github.com/casey/just)** (optional): `cargo install just`.
- **macOS**: Command Line Tools are enough because `stet-ui` enables GPUI's
  `runtime-shaders` feature. If you install full Xcode and prefer shaders
  compiled at build time, use `cargo run -p stet --no-default-features`.
- **Linux**: GPUI needs windowing and font development packages, e.g. on
  Debian/Ubuntu:

  ```sh
  sudo apt install libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev \
      libx11-dev libxcb1-dev libfontconfig1-dev libfreetype-dev
  ```

- **Windows**: MSVC toolchain; nothing extra.

## Everyday commands

```sh
just run                    # launch the editor
just test                   # full workspace tests
cargo test -p stet-core     # fast path while working on the engine
just lint                   # clippy, -D warnings
just fmt                    # rustfmt
just ci                     # what CI runs: fmt-check, lint, test
just deny                   # dependency policy (licenses/advisories)
just build                  # release binary at target/release/stet
```

## Repository layout

See the repo map in [AGENTS.md](../AGENTS.md). The short version:
`stet-core` is the engine, `stet-syntax` the grammar layer, `stet-plugin`
the plugin contracts, `stet-ui` the GPUI shell, `apps/stet` the binary.

## Testing strategy

- **`stet-core`**: unit tests in each module cover positions, graphemes,
  word boundaries, CRLF, edits and undo. Every logic change needs a test.
- **`stet-syntax`**: per-language tests assert on real highlighted spans.
- **`stet-plugin`**: manifest parsing/validation tests.
- **`stet-ui`**: headless GPUI tests (`gpui/test-support`) cover typing,
  IME composition, auto-pairs and UTF-16 ranges through the real input
  handler. Workspace-level tests are still on the roadmap; for other UI
  changes, smoke-test by running the app and describe what you did in the
  PR.

## Adding a language

1. `cargo add tree-sitter-<lang>` in the workspace and add the dependency
   to `crates/stet-syntax/Cargo.toml`.
2. Add a variant to `Language`, an extension mapping, and a config arm in
   `crates/stet-syntax/src/language.rs`. Grammar crates expose `LANGUAGE`
   (a `LanguageFn`), `HIGHLIGHTS_QUERY` and sometimes `INJECTIONS_QUERY`.
3. Add a test to `crates/stet-syntax/src/highlight.rs` that asserts one
   expected span (e.g. a keyword).
4. Update the README feature list.

## Debugging tips

- GPUI ships an inspector (`gpui::inspect()`) that shows element styles and
  bounds; Stet has not wired a keybinding for it yet — add a temporary call
  when chasing layout bugs.
- Editor layout problems are usually `LINE_HEIGHT`/`GUTTER_WIDTH`
  mismatches between `editor.rs` and the row wrappers in `workspace.rs`.
- If the app opens but typing does nothing, check focus: GPUI focus is
  explicit, and `workspace.rs` must `window.focus(...)` the editor after
  tab switches, palette closes and file opens.
- If text positions look wrong after mouse clicks, the hit-test map in
  `EditorView` was built from a stale frame; confirm lines are being
  recorded in `LineElement::paint`.

## Dependency policy

- Direct dependencies need a reason. Prefer the standard library and
  existing deps.
- `gpui` is pinned exactly; other deps use caret requirements. Upgrades to
  `gpui` require running the app and reviewing the `Element` impl in
  `editor.rs`.
- `cargo deny` gates licenses (permissive only) and known vulnerabilities.
  Adding a dependency with a new license means updating `deny.toml` in the
  same PR.
- Keep the `unsafe` ban: workspace crates set `unsafe_code = "forbid"`.

## Release builds

`just build` produces a stripped-ish release binary with thin LTO. Cross
compilation is not set up yet; each platform builds natively in CI.
