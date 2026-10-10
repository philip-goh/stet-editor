## What changed

<!-- One or two sentences. Link the issue if there is one. -->

## How it was verified

- [ ] `just ci` passes locally
- [ ] New/updated tests cover the change (or: why not)
- [ ] For UI changes: I ran `just run` and checked <!-- describe what -->

## Checklist

- [ ] Dependency direction respected (no UI types in `stet-core`)
- [ ] Commands go through a single `perform_*` implementation
- [ ] Public items documented; comments explain *why*
- [ ] ADR added/superseded for architectural decisions
- [ ] Roadmap checkboxes updated if a roadmap item is complete
- [ ] `just deny` run if dependencies changed
