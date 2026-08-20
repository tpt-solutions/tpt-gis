# AGENTS.md

Compact, command-focused guidance for agents in this repo. Full conventions live in
`CLAUDE.md`; read it for architecture and testing rationale.

## Workspace layout (root `Cargo.toml`)
- Five library crates: `tpt-gis-core`, `tpt-gis-geom` (both `no_std` by default;
  `std` is an opt-in feature flag), `tpt-gis-io`, `tpt-gis-index`, `tpt-gis-raster`.
- `tpt-gis` — facade re-exporting the five crates behind feature flags. Default =
  all features, including `http` (which pulls in reqwest/tokio).
- Three example binaries: `examples/drone-geofence-demo`, `examples/spatial-join-cli`,
  and `examples/wasm-geofence-demo` (the latter is excluded from the workspace — see below).
- Excluded from the workspace (`exclude` in root `Cargo.toml`) — `cargo build
  --workspace` / `cargo test --workspace` will NOT build these:
  - `examples/wasm-geofence-demo` — `cdylib`, builds only for
    `wasm32-unknown-unknown` (needs `wasm-bindgen` + `wasm-bindgen-cli`).
  - `tpt-gis-io/fuzz`, `tpt-gis-raster/fuzz` — cargo-fuzz harnesses; need `nightly`
    + `cargo-fuzz` (`cargo fuzz build -O`), not local stable.

## Commands (exact, mirror `.github/workflows/ci.yml`)
- Build + test: `cargo build --workspace --all-targets` then `cargo test --workspace`
- Lint gate (must be clean): `cargo clippy --workspace --all-targets -- -D warnings`
- Format: `cargo fmt --all -- --check`
- License/advisory (needs `cargo install cargo-deny`): `cargo deny check`
- Single test: `cargo test -p tpt-gis-core <module>::tests::<name>`
- `no_std` cross-check: `cargo build -p tpt-gis-core -p tpt-gis-geom --target thumbv7em-none-eabihf`
  (also `riscv32imc-unknown-none-elf` = ESP32-C3, `wasm32-unknown-unknown`, `wasm32-wasip1`)
- Benches: `cargo bench -p spatial-join-cli`, `cargo bench -p drone-geofence-demo`,
  `cargo bench -p tpt-gis-raster --features http --bench cog_stream`

## Conventions that bite
- `no_std` discipline: `tpt-gis-core`/`tpt-gis-geom` use `libm` (e.g. `libm::sqrt(x)`),
  never `std` `f64` methods, and must stay `no_std` cross-compilable. Re-check any
  change to them against the embedded/wasm targets above.
- Workspace lints (`[workspace.lints]`): `unsafe_code = "deny"` (avoid unsafe; needs a
  `// SAFETY:` comment + discussion), `missing_docs = "warn"`, and
  `clippy::all = "warn"`. Only `clippy::all` is enforced — NOT `clippy::pedantic`.
- `clippy --all-targets -- -D warnings` covers tests/benches. Keep dead code,
  redundant `as` casts, manual `div_ceil`, and single-arm `match` out, or CI fails.
- Every dependency must be MIT/Apache-2.0-compatible (zero GPL/LGPL); `cargo-deny`
  enforces this against `deny.toml`. Verify any new dep before adding.

## Correctness traps
- `tpt-gis-index/src/s2.rs` is a self-consistent Z-order (Morton) cell scheme. It is
  NOT bit-compatible with Google's S2 (Hilbert curve). Do not add tests asserting
  equality with Google S2 cell ids/tokens.
- `tpt-gis-io` has GeoPackage (`.gpkg`) reader/writer in `tpt-gis-io/src/geopackage/`
  (pure-Rust, zero-FFI SQLite file format + GeoPackageBinary geometry codec). It is
  read-capable for leaf/interior table b-trees and write-capable for single-leaf-page
  tables only — see that module's docs for scope limits.
- Geodesy/geometry changes need a reference-vector or provable-invariant test, not
  just a round-trip (see the Vincenty Flinders-Peak→Buninyong and UTM
  central-meridian tests in `tpt-gis-core`).
- Untrusted parsers (GeoTIFF/HTTP/Shapefile/WKB/WKT/GeoJSON) must bound allocations
  and reject non-finite coordinates and deeply-nested input. See `todo.md`.
