# 0003. Build the UI on GPUI, not Floem

- Status: Accepted
- Date: 2026-10-10

## Context

The UI framework is the single hardest dependency to reverse, so we
evaluated the realistic options for a GPU-rendered, cross-platform editor in
Rust. The candidates:

| | GPUI (Zed) | Floem (Lapce) | Custom wgpu stack | egui / iced |
| --- | --- | --- | --- | --- |
| Activity (as of Oct 2026) | ~428 commits to `crates/gpui` since Jan 2026 in zed-industries/zed; releases on crates.io (0.2.2) | ~64 commits since Jan 2026; no crates.io release since 0.2.0 (Nov 2024); Lapce itself very active | n/a | Very active |
| Backing | Funded company (Zed Industries), weekly editor releases | One principal consumer (Lapce) | Us | Large communities |
| Text editing support | Framework only; Zed's editor crates are unpublished, so the text surface must be built | Ships `floem-editor-core`, a full IME-aware editor widget | Everything ours | Weak for large-document editing |
| API stability | 0.x, moves fast | 0.x, slow releases (staleness risk) | Ours | Better |
| Fit for this project | Best velocity/ecosystem, more work on the text surface | Fastest path to a usable editor, lower ecosystem momentum | Maximum control, months before usable | Not suited to a serious editor |

The deciding factors were project momentum and ecosystem: GPUI is developed
continuously by a company whose product is exactly the kind of editor we are
building, and community adoption is an order of magnitude larger. The cost —
writing the text surface ourselves — is work we would eventually have to do
anyway, because a Sublime-class editor cannot be built on a borrowed text
widget long-term.

## Decision

Build `stet-ui` on `gpui = "0.2.2"` (pinned exactly), with GPUI's
`runtime_shaders` feature enabled by default so that macOS machines with
only Command Line Tools can build.

The text surface is ours: `EditorView` (state) and `LineElement` (a custom
`Element` that shapes, highlights and paints lines), virtualized with
`gpui::uniform_list`. `stet-core` remains UI-agnostic (ADR-0002), so the
editor model is not coupled to GPUI's entity system.

GPUI usage is confined to `stet-ui`. The rest of the workspace must not
import it.

## Consequences

- We own IME, selection rendering, hit-testing and scroll behaviour. The
  first iteration is deliberately minimal (ADR-ext external limitations are
  listed in docs/architecture.md).
- GPUI upgrades are a contained, scheduled task: bump, run the app, review
  `LineElement` against the `Element` trait and check shader features.
- CI compiles GPUI on all three desktop platforms; Linux needs a small set
  of windowing/font dev packages.

## Exit criteria and migration path

This decision is revisited if any of the following holds:

1. GPUI has no commits for 6 months, or Zed stops shipping desktop builds.
2. GPUI publishes breaking changes faster than we can absorb (more than one
   forced migration per quarter for two consecutive quarters).
3. A platform we need (e.g. Android/iPadOS) is not supportable by GPUI
   while remaining supportable elsewhere.

Migration path, cheapest first: (a) pin an older GPUI and defer upgrades;
(b) fork GPUI at the pinned revision (Apache-2.0 permits it) and maintain
the fork in-house; (c) port `stet-ui` to another framework — viable because
`stet-core` is UI-agnostic and `stet-ui` is the only GPUI consumer.

## Alternatives considered

- **Floem**: the strongest alternative and initially selected. Its editor
  widget would have delivered IME and selection for free, but the project's
  release cadence has stalled since 2024 and it is effectively maintained
  for one consumer. Betting a multi-year editor on it concentrates risk.
- **Custom winit + wgpu + cosmic-text**: maximum control and performance,
  but months of work on windows, input, accessibility and text layout
  before the first keystroke lands. Rejected for v0.1; the UI-agnostic core
  keeps this door open.
- **egui / iced**: active ecosystems, but immediate-mode redraw and lack of
  serious large-document text infrastructure make them a poor fit for this
  product.
