# Crates.io Publishing Checklist

This document tracks the per-crate steps required before publishing to crates.io.

## Naming / Namespace

All crates are prefixed `tpt-gis-` to avoid namespace collisions:
- `tpt-gis-core`
- `tpt-gis-geom`
- `tpt-gis-raster`
- `tpt-gis-io`
- `tpt-gis-index`

Reserve these names on crates.io before the first publish. If any are taken, fall
back to `tpt-gis-<crate>-rs` (e.g. `tpt-gis-core-rs`).

## Pre-publish Checklist

For each crate:

1. **README**: Include a short description, install instructions, and a minimal example.
2. **license / repository / homepage**: Set in `Cargo.toml` (already done via workspace).
3. **documentation**: Run `cargo doc --no-deps` and verify no warnings.
4. **tests**: Run `cargo test --workspace` and confirm all pass.
5. **clippy**: Run `cargo clippy --all-targets -- -D warnings`.
6. **version**: Bump version in root `Cargo.toml` workspace package.
7. **changelog**: Add an entry to `CHANGELOG.md`.
8. **git tag**: `git tag -a vX.Y.Z -m "Release X.Y.Z" && git push --tags`.

## Publish Order

Publish in dependency order:
1. `tpt-gis-core`
2. `tpt-gis-geom`
3. `tpt-gis-io`
4. `tpt-gis-index`
5. `tpt-gis-raster` (Phase 3)

Example crates and benches (`drone-geofence-demo`, `spatial-join-cli`) should NOT
be published (`publish = false` in their `Cargo.toml`).
