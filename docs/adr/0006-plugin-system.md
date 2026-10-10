# 0006. WASM-first, capability-based plugins

- Status: Accepted
- Date: 2026-10-10
- Related: ADR-0007 (VS Code compatibility) builds on this decision.

## Context

Plugins are the difference between an editor and *your* editor. They are
also an arbitrary-code-execution vector and a per-platform build problem.
Options considered:

- **Native dylibs** (Rust ABI or C ABI): fastest, but unsafe (plugins share
  the process), unstable ABI across Rust versions, and a per-platform build
  matrix for plugin authors.
- **Embedded JS engine** (QuickJS/Boa): the eventual requirement for VS
  Code compatibility, but as the *primary* API it forces every plugin
  through a slow runtime and a large embed.
- **WebAssembly**: sandboxed by construction, portable, language-agnostic,
  with maturing component-model tooling.

## Decision

The plugin system is WASM-first:

1. Plugins are WebAssembly modules executed by a hosted runtime in a future
   `stet-plugin-host` crate (wasmtime or extism; the choice gets its own
   ADR when the host lands).
2. The host interface is **capability-based**: plugins get exactly the
   capabilities the editor hands them — no ambient filesystem, network or
   editor internals. The Rust-side contract is `stet_plugin::Host`; the
   cross-boundary ABI will be defined in WIT.
3. Plugins are described by a declarative manifest (`stet-plugin.json`,
   modeled on the useful half of VS Code's `package.json`) so activation
   and static contributions are known before any code runs.
4. The editor only ever talks to plugins through the `Host`/`Plugin`
   traits. The WASM runtime is an implementation detail, which is what
   makes the VS Code bridge (ADR-0007) a translation layer rather than a
   redesign.

Execution is off the UI thread once the host exists; plugin calls return
plain data, never live editor references.

## Consequences

- Plugin crashes and infinite loops are contained; no shared-memory ABI
  hazards.
- Plugin authors can use any language that compiles to WASM.
- The runtime adds a heavy dependency and startup cost; it is isolated in
  the host crate so the editor binary does not pay for it until plugins
  load.
- WASM cannot express everything an editor API eventually needs (rich text
  edits, diagnostics streams); the WIT interface will grow deliberately,
  with compatibility rules.

## Alternatives considered

- **Native dylibs**: rejected on safety and ABI-stability grounds.
- **JS-first**: rejected as the primary contract; it remains a second
  execution backend behind the same host traits to serve VS Code compat.
- **Process-isolated native plugins**: safer than dylibs but a packaging and
  IPC burden with no portability win over WASM.
