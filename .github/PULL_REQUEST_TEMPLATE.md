## What

Briefly describe the change.

## Why

What problem does this solve, or which `todo.md` item does this complete?

## Checklist

- [ ] `cargo test --workspace` passes
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] If `tpt-gis-core` or `tpt-gis-geom` changed: still builds for
      `thumbv7em-none-eabihf` and `wasm32-unknown-unknown`
- [ ] New/changed geodesy or projection math includes a test against an
      independently verifiable reference (published test vector or provable
      invariant), not just a round-trip check
- [ ] No new GPL/LGPL-licensed dependencies
- [ ] Relevant `todo.md` checkbox(es) updated
