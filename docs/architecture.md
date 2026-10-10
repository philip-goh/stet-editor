# Architecture

This document is the map; the [ADRs](adr/) are the law. If they disagree,
the ADRs win and this document has a bug.

## Crate graph

```
          apps/stet (binary)
               |
               v
          stet-ui (GPUI)
          /    |     \
         v     v      v
   stet-core  stet-syntax  stet-plugin
         ^         |
         +---------+
```

Dependencies point one way and `stet-core` has no UI dependency at all
([ADR 0002](adr/0002-editor-core-is-ui-agnostic.md)).

## The three layers

### 1. `stet-core` — the text engine

Everything that is true independent of a screen lives here:

- `TextBuffer` — a [rope](adr/0004-text-buffer.md) with byte-offset
  access, char/line/column conversion, grapheme- and word-boundary
  navigation, and an `edit()` that returns an `AppliedEdit` describing both
  how to undo the change and how tree-sitter should adjust its parse.
- `Document` — buffer + path + content-hash dirty state + undo history +
  atomic save. UTF-8 only for now (BOM stripped), line endings preserved.
- `Selection` — anchor/head byte offsets. The head is the caret.
- `History` — undo/redo over `UndoStep`s: one or more `AppliedEdit`s that
  undo together. Insertion and deletion runs coalesce; batched operations
  (block indent, and later multi-cursor typing and replace-all) record a
  transaction so they undo as one unit.

Rules of the layer:

- Offsets are **bytes**. Anything named `*_chars` or `Position::column` is
  the exception and says so.
- Out-of-bounds offsets are clamped, never fatal: stale UI offsets must not
  crash the editor.
- No GPUI, no `ElementId`, no colors. Data in, data out.

### 2. Supporting services

**`stet-syntax`** maps paths to grammars ([ADR 0005](adr/0005-syntax-highlighting.md))
and owns one `Highlighter` per document. `refresh(text)` re-parses and
rebuilds a per-line cache of `HighlightSpan`s (`start..end` bytes relative
to the line, plus a coarse `HighlightKind`). The UI asks per visible line
and never drives the parser. Documents over 1 MiB are deliberately not
highlighted until background highlighting lands.

**`stet-plugin`** defines the plugin contract and nothing else: manifests
(`stet-plugin.json`), capability traits (`Host`), and the plugin lifecycle
(`Plugin`). The WASM host that executes modules is a separate future crate
([ADR 0006](adr/0006-plugin-system.md)); the VS Code bridge is a translation
layer on top ([ADR 0007](adr/0007-vscode-extension-compatibility.md)).

### 3. `stet-ui` — GPUI

- `Workspace` is the root entity: tabs, toolbar, status bar, palette, and
  **all action handlers**. Every command has exactly one `perform_*`
  implementation reached by keys, buttons and palette alike.
- `EditorView` is one entity per tab. It owns the `Document`, selection,
  highlighter and scroll position, and exposes plain methods
  (`insert_text`, `move_vertical`, `undo`, ...).
- `LineElement` is a custom GPUI `Element` that shapes one visible line with
  syntax-colored runs, paints selection quads and the caret, and records
  its bounds back into `EditorView` so mouse hit-testing can convert pixels
  to byte offsets.
- Lines are virtualized with `gpui::uniform_list`; scrolling, hit-testing
  and cursor visibility all key off the visible line range.

## Data flow of a keystroke

```
platform key event
  -> GPUI keymap resolves an action (arrows, cmd-s, ...)  -> Workspace::on_*
  -> otherwise KeyDown reaches the focused editor          -> EditorView::handle_key_down
  -> EditorView::insert_text / delete_backward / ...
  -> Document::edit -> TextBuffer::edit (rope) + History::record
  -> Highlighter::refresh on the new text
  -> cx.notify() -> Workspace re-renders affected lines
```

Paste, clipboard and file dialogs go through GPUI's native platform APIs
(`read_from_clipboard`, `prompt_for_paths`, `prompt_for_new_path`) — there is
no custom dialog UI in v0.1.

## Threading model

v0.1 is single-threaded on the UI thread, like most editors at this stage.
The two planned escape hatches, in order:

1. **Background highlighting**: `Highlighter` moves to an executor-owned
   worker, returning line spans to the UI. The data types are already
   `Send`-friendly plain data.
2. **Plugin host**: WASM executes off-thread behind the capability API.

## Performance principles

- Painting touches only visible lines; shaping happens in prepaint.
- Highlights are cached per line and recomputed per parse, not per frame.
- Undo/redo is edit-log based; no document snapshots.
- The rope is the only copy of the text in `stet-core`; the UI does not
  keep a shadow buffer.

## Deliberate v0.1 limitations

Tracked in the [roadmap](roadmap.md), listed here so nobody mistakes them
for bugs:

- IME/composition is not wired (`key_char` only).
- No multi-cursor, no find/replace, no line wrapping.
- Highlighting is synchronous and size-limited.
- No settings file; the action list is static.
