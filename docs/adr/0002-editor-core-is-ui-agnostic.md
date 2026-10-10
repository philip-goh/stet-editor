# 0002. Keep the editor core UI-agnostic

- Status: Accepted
- Date: 2026-10-10

## Context

Editor bugs concentrate in the text engine: offsets, graphemes, selections,
undo. The UI framework (GPUI) is the least stable and most expensive
dependency in the project; its API changes on every release. If core logic
is entangled with UI types, then (a) engine tests need a GUI context,
(b) framework churn becomes engine churn, and (c) a future framework
migration or a second frontend (mobile, tests, CLI tools) is impossible.

## Decision

`stet-core` contains no UI dependency — no GPUI, no `ElementId`, no colors,
no window types. It exposes plain data APIs:

- byte-offset editing (`TextBuffer::edit`) returning plain-data results
- position conversion (`Position` with char columns) as pure functions
- documents with plain paths and revision counters
- selections as anchor/head byte offsets

UI concerns (virtualization, shaping, hit-testing, dirty markers) live in
`stet-ui`. Where UI code needs engine data, the engine grows a data API,
not a callback into the UI.

Enforced by the crate graph: `stet-core`'s `Cargo.toml` must not gain a UI
dependency, and `unsafe` is forbidden workspace-wide.

## Consequences

- Engine tests run headless and fast (`cargo test -p stet-core`).
- GPUI upgrades touch only `stet-ui`.
- A future web/mobile/CLI frontend can reuse the engine.
- Slightly more plumbing: e.g. the UI reads `cursor_status()` rather than
  the core knowing about status bars.

## Alternatives considered

- **UI types in core**: convenient short-term, couples the most valuable
  code to the most volatile dependency.
- **A trait-based "view" abstraction in core**: speculative; adds
  indirection without a second consumer.
