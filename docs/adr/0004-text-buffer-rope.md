# 0004. Rope-backed text buffer with `ropey`

- Status: Accepted
- Date: 2026-10-10

## Context

The buffer must support: insertions anywhere (typing), line-indexed access
for rendering and gutters, byte↔char↔line conversions, multi-megabyte
documents, and cheap slicing for saving and highlighting. The classic
choices are gap buffers, piece tables, and ropes.

Requirements that matter here:

- Random access by line and byte offset for the UI and tree-sitter.
- Efficient edits at the caret, including large pastes.
- Mature, documented, permissively licensed implementation.

## Decision

Use [ropey](https://github.com/cessen/ropey) 1.6 as the storage for
`TextBuffer`. All offsets crossing the core API are byte offsets;
`ropey`'s char indices are used only inside conversions
(`byte_to_char`, `line_to_char`, ...).

The buffer stores text exactly as loaded and remembers the dominant line
ending (`LineEnding::Lf` / `CrLf`); inserted newlines follow it. Unicode
grapheme and word boundaries come from `unicode-segmentation`, applied
line-locally so a multi-byte character can never be split.

`ropey` 2.0 is in beta; we stay on the stable 1.6 line until 2.0 is stable
and we have a reason to move.

## Consequences

- Edits are O(log n); line and byte indexing are fast enough for
  virtualization at 60+ fps.
- Out-of-bounds access is clamped in our wrapper, so stale UI offsets never
  panic.
- Rope metrics (line count includes a trailing empty line after a final
  newline) leak into test expectations and are documented in `text.rs`.
- We carry an explicit dependency on `unicode-segmentation` for grapheme
  and word boundaries; behavior is tested with combining characters.

## Alternatives considered

- **`crop`** (Zed's rope): excellent performance and used in production by
  Zed, but less documentation and a smaller independent community. A
  plausible future move; nothing in our API would change.
- **Piece table**: great for undo-heavy editing, but line indexing and
  slicing are more work and undo is already edit-log based, so the piece
  table's advantage does not apply.
- **Gap buffer**: simple and cache-friendly, but terrible worst-case for
  large documents with edits far from the gap.
- **`String` + no rope**: acceptable for toy editors, unacceptable for the
  product goal.
