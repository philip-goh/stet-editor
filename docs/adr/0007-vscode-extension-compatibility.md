# 0007. VS Code extension compatibility strategy

- Status: Accepted
- Date: 2026-10-10
- Depends on: ADR-0006 (WASM-first plugin host)

## Context

VS Code's extension ecosystem is its moat: tens of thousands of extensions
are a large part of why people stay. Stet cannot run them natively — they
are JavaScript/TypeScript modules running in a Node.js extension host,
talking to an Electron UI over a typed API (`vscode`), with a declarative
manifest (`package.json` `contributes`) and Node built-ins available.

Two facts shape any strategy:

1. The *declarative* half (commands, menus, keybindings, languages, themes,
   configuration schemas) is data. It can be translated completely.
2. The *imperative* half (`activate()`, `workspace.onDidChangeTextDocument`,
   providers, webviews) is code against a large API surface. Some of it is
   impossible by design (Electron-specific UI), some is mechanical.

A full Node-compatible host is a multi-year project on its own and would
drag Node's runtime model into a deliberately light editor.

## Decision

Pursue compatibility in stages, each stage independently useful, all of it
translating into the ADR-0006 plugin primitives so the rest of the editor
never special-cases extension provenance:

1. **Manifest translation (M4)**: parse `.vsix` archives, translate
   `package.json` into `PluginManifest` (`engines` → engine constraint,
   `contributes.commands`/`languages`/`configuration` → contributions,
   `activationEvents` → `ActivationEvent`). Declarative-only extensions
   (themes, snippets, keybindings) work at this stage with zero code
   execution.
2. **API shim over WASM (M4)**: define the `vscode` module API as a
   capability interface; generated bindings expose it to extensions running
   in the WASM host. Text document events map onto `Host`'s document
   snapshot API. Extensions that only use documents, workspace, commands
   and messages port at this stage.
3. **JS execution backend (M4+)**: add a JavaScript runtime (QuickJS-class)
   behind the same host traits for extensions that ship JavaScript and
   Node-ish dependencies. `require('vscode')` resolves to the shim; a
   curated Node built-in subset (`path`, `events`, `util`) is provided.
   This is where the bulk of real-world extensions become runnable.
4. **Honest gaps**: webviews, Electron-only APIs and native Node addons are
   out of scope and must fail with clear, surfaced errors rather than
   silently misbehave.

The compatibility target is a curated set of popular extensions, tested in
CI with recorded fixtures, not "all of the marketplace".

## Consequences

- Declarative extensions and themes can arrive years before the JS host is
  viable, because step 1 needs no runtime.
- The `vscode` shim and the WASM ABI must be co-designed: the shim is a
  facade over `Host` capabilities, which keeps the security model intact.
- We will maintain a compatibility matrix and a known-unsupported list;
  pretending otherwise would burn user trust quickly.
- VS Code *themes* use TextMate scopes, while our highlighting is
  tree-sitter (ADR-0005). Theme translation needs a scope→`HighlightKind`
  mapping table; a follow-up ADR will specify it when themes land.

## Alternatives considered

- **Fork VS Code's extension host (Node) outright**: maximal compatibility,
  destroys the lightweight goal and pulls in Node's security and packaging
  model.
- **Do nothing**: acceptable for M3, but it forfeits the single biggest
  ecosystem advantage available to a new editor.
- **Transpile JS to WASM (e.g. via a JS engine compiled to WASM)**: a
  promising efficiency play for step 3; keep the backend abstract so this
  can be swapped in without touching the shim.
