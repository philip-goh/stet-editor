# Contributing to Stet

Thanks for helping build Stet. This guide covers the practical workflow;
[AGENTS.md](AGENTS.md) has the condensed conventions and
[docs/development.md](docs/development.md) the tooling details.

## Before you start

- Rust is pinned by `rust-toolchain.toml`; `rustup` installs it on first use.
- Linux contributors need a few system libraries; see
  [docs/development.md](docs/development.md).
- `just` is optional but recommended (`cargo install just`).

## Workflow

1. **Discuss first for large changes.** Open an issue describing the change.
   Editor architecture is easy to get subtly wrong, and reviewers care more
   about the approach than the diff.
2. **Branch** off `main` with a descriptive name (`fix/crlf-navigation`,
   `feat/command-palette-scoring`).
3. **Make the change**. Follow the golden rules in [AGENTS.md](AGENTS.md).
4. **Add tests**. Logic changes need unit tests; UI changes need a described
   smoke test.
5. **Run the checks**:

   ```sh
   just ci     # fmt-check + clippy -D warnings + tests
   just deny   # when dependencies changed
   ```

6. **Open a pull request** using the template. Keep it focused: one logical
   change per PR.

## Commit messages

- Imperative mood, sentence case, no trailing period: `Add word-wise
  navigation to the editor`.
- Prefix with the crate when it helps: `stet-core: fix CRLF line ends`.
- Explain *why* in the body when the diff does not.

## Architecture decisions

Any decision that is expensive to reverse (dependencies, data formats,
process boundaries, public APIs) needs an ADR. Copy the template in
[docs/adr/README.md](docs/adr/README.md), use the next number, and land it
in the same PR as the change. Never edit an accepted ADR; write a new one
that supersedes it.

## Code style

- `cargo fmt` decides formatting. `cargo clippy -- -D warnings` decides the
  rest.
- Public items need doc comments; explain *why*, not *what*, in inline
  comments.
- Prefer plain data and pure functions in `stet-core`; keep GPUI types out
  of it.

## Review checklist

Reviewers check, in order:

1. Correctness of the data model (byte offsets, selections, undo).
2. Tests that would fail without the change.
3. Dependency direction and the golden rules.
4. Docs and ADRs when behaviour or architecture changed.
