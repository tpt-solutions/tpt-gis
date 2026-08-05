# Crates.io Publishing Checklist

This document tracks the per-crate steps required before publishing to crates.io.

## Naming / Namespace

All crates are prefixed `tpt-gis-` to avoid namespace collisions:
- `tpt-gis-core`
- `tpt-gis-geom`
- `tpt-gis-raster`
- `tpt-gis-io`
- `tpt-gis-index`
- `tpt-gis` (facade crate — publish **last**)

Reserve these names on crates.io before the first publish. If any are taken, fall
back to `tpt-gis-<crate>-rs` (e.g. `tpt-gis-core-rs`).

## Pre-publish Checklist

For each crate:

 1. **README**: Include a short description, install instructions, and a minimal example.
 2. **license / repository / homepage**: Set in `Cargo.toml` (already done via workspace).
 3. **keywords / categories**: Set in `Cargo.toml` for crates.io discoverability (added to
    all five library crates).
 4. **documentation**: Run `cargo doc --no-deps` and verify no warnings.
 5. **tests**: Run `cargo test --workspace` and confirm all pass.
 6. **clippy**: Run `cargo clippy --all-targets -- -D warnings`.
 7. **version**: Bump version in root `Cargo.toml` workspace package.
 8. **changelog**: Add an entry to `CHANGELOG.md`.
 9. **git tag**: `git tag -a vX.Y.Z -m "Release X.Y.Z" && git push --tags`.

## Publish Order

Publish in dependency order:
1. `tpt-gis-core`
2. `tpt-gis-geom`
3. `tpt-gis-io`
4. `tpt-gis-index`
5. `tpt-gis-raster` (Phase 3)
6. `tpt-gis` (facade) — published last, after all engine crates are on crates.io

Example crates and benches (`drone-geofence-demo`, `spatial-join-cli`) should NOT
be published (`publish = false` in their `Cargo.toml`).
