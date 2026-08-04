# Contributing to tpt-gis

Thanks for your interest in contributing. This project is in its early phases
(see [`todo.md`](todo.md)) — the fastest way to help is usually to pick up an
unchecked item there, or open an issue to discuss a larger change before writing
code.

## License

`tpt-gis` is dual-licensed MIT / Apache-2.0. By submitting a contribution, you
agree to license it under both, per the standard Rust ecosystem convention (see
the note at the bottom of the [README](README.md#license)).

**Zero GPL/LGPL dependencies, strictly enforced.** Any new dependency must be
compatible with permissive (MIT/Apache-2.0-style) licensing — this is a hard
requirement, not a preference, since it's core to the project's value
proposition for commercial and government adopters. CI runs `cargo-deny` to
enforce this; a dependency that fails the license check will not be merged
until it's replaced or the license is confirmed compatible.

## Development setup

```sh
cargo build --workspace
cargo test --workspace
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

`tpt-gis-core` and `tpt-gis-geom` are `no_std` by default. If you touch either
crate, also check it still cross-compiles without `std`:

```sh
cargo build -p tpt-gis-core -p tpt-gis-geom --target thumbv7em-none-eabihf
cargo build -p tpt-gis-core -p tpt-gis-geom --target wasm32-unknown-unknown
```

## Code style

- No `unsafe` (denied at the workspace level) unless there is no safe alternative
  and it comes with a `// SAFETY:` comment justifying it, plus explicit discussion
  in the PR.
- Public items need doc comments (`missing_docs` is a warning, trending toward
  deny as coverage improves).
- Prefer borrowed data (`&[Point]`) over owned/heap types in `tpt-gis-core` and
  `tpt-gis-geom`, so they stay usable on embedded targets with zero allocation.
- New geodesic/projection math should include a test against an independently
  verifiable reference value (a published test vector, or a provable invariant of
  the formula), not just a round-trip check — see `tpt-gis-core/src/geodesic.rs`
  and `tpt-gis-core/src/projection/utm.rs` for examples of both.

## Pull requests

- Keep PRs focused; large architectural changes should start as an issue/discussion.
- Include tests for new behavior and update `todo.md` checkboxes for whatever the
  PR completes.
- CI (build, test, fmt, clippy, license check) must pass before merge.

## Code of Conduct

This project follows the [Code of Conduct](CODE_OF_CONDUCT.md). Please read it
before participating.
