# 0001. Record architecture decisions

- Status: Accepted
- Date: 2026-10-10

## Context

Stet is a long-lived project with an ambitious scope (text engine, UI,
syntax, plugins, eventually VS Code extensions). Most costly mistakes in
projects like this are architectural and get made silently: a dependency
direction, a data format, a threading model. New contributors — human or AI
— need to know not just what the code does but why it is shaped that way.

## Decision

Every decision that is expensive to reverse gets a numbered ADR in this
directory, using the template in [README.md](README.md). Accepted ADRs are
immutable; changing a decision means writing a superseding ADR.

Examples of decisions that require an ADR:

- Dependencies with broad impact (UI framework, rope, parser, WASM runtime)
- Process/thread boundaries and data formats
- Public API contracts and plugin ABI changes
- Anything that violates a previously accepted ADR

## Consequences

- The repository carries a short, readable history of *why*.
- Reviewers can point at decisions instead of re-litigating them.
- Slight overhead: an ADR is expected alongside structural PRs.

## Alternatives considered

- **Comments in code**: disappear from view and rot.
- **A wiki**: drifts from the code and is not reviewed with PRs.
- **One big architecture doc**: becomes unmaintainable and hides the "why".
