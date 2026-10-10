# Stet

A cross-platform text editor written in Rust, sitting between Sublime Text's
lightness and VS Code's capability. GPU-rendered, fast, resource-light, and
built to grow a real plugin ecosystem.

> **Status: v0.1 (walking skeleton).** Open, edit and save files with
> syntax highlighting, tabs and a command palette. The architecture is in
> place for everything that follows; see [docs/roadmap.md](docs/roadmap.md)
> for what is next.

## Features

- Native GPU-accelerated UI via [GPUI](https://github.com/zed-industries/zed)
- Rope-backed text engine with grapheme-aware editing and grouped undo
- Real IME support: CJK composition, dead keys, preedit rendering
- Tree-sitter syntax highlighting (Rust, JavaScript, JSON, TOML)
- Auto-closing brackets and quotes, word/line selection, block indent
- Tabs, command palette, status bar
- Atomic file saves, UTF-8 BOM handling, LF/CRLF preservation
- Sandboxed WASM-first plugin ABI (contracts in place, host on the roadmap)

## Building

Requires Rust (pinned by `rust-toolchain.toml`).

```sh
just run                       # or: cargo run -p stet
cargo run -p stet -- file.rs   # open a file
```

On macOS, GPUI normally compiles Metal shaders with `xcrun metal`, which
requires full Xcode. `stet-ui` defaults to the `runtime-shaders` feature so
the editor also builds with only Command Line Tools; if you have Xcode and
prefer build-time shaders, use `cargo run -p stet --no-default-features`.
See [docs/development.md](docs/development.md) for Linux prerequisites.

## Project layout

| Crate | Purpose |
| --- | --- |
| `crates/stet-core` | UI-agnostic text engine: buffers, edits, selections, undo, documents |
| `crates/stet-syntax` | Language detection and tree-sitter highlighting |
| `crates/stet-plugin` | Plugin manifest and WASM host ABI contracts |
| `crates/stet-ui` | GPUI views: editor surface, tabs, palette, theme |
| `apps/stet` | The binary |

Dependencies point one way: `apps/stet -> stet-ui -> { stet-core, stet-syntax,
stet-plugin }`. `stet-core` never depends on UI (see
[docs/adr/0002](docs/adr/0002-editor-core-is-ui-agnostic.md)).

## Documentation

- [Architecture](docs/architecture.md)
- [Development guide](docs/development.md)
- [Roadmap](docs/roadmap.md)
- [Architecture decision records](docs/adr/)
- [Contributing](CONTRIBUTING.md)
- [AGENTS.md](AGENTS.md) — orientation for AI coding agents

## License

[MIT](LICENSE)
