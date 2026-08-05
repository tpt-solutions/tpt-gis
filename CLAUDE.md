# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project overview

`tpt-gis` is a pure Rust, `no_std`-capable, planetary-scale GIS engine intended as a
zero-FFI replacement for the GDAL/PROJ/GEOS stack. It is a Cargo workspace of five
crates, now well into its roadmap (see `todo.md` for the full checklist and `spec.txt`
for the design rationale). All five crates have real implementations:

```
tpt-gis-core      # CRS definitions, datum transforms, geodesic math — no_std, implemented
tpt-gis-geom      # OGC Simple Features geometry + topological predicates — no_std, implemented
tpt-gis-io        # GeoJSON/WKB/WKT/Shapefile readers+writers — implemented, Phase 2
tpt-gis-index     # R-Trees, Quadtrees, H3/S2 — implemented, Phase 2
tpt-gis-raster    # Grid data, map algebra, COG/GeoTIFF parsing — implemented, Phase 3
```

## Commands

```sh
cargo build --workspace
cargo test --workspace
cargo fmt --all
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Run a single test: `cargo test -p tpt-gis-core geodesic::tests::vincenty_inverse_flinders_peak_to_buninyong`

`tpt-gis-core` and `tpt-gis-geom` are `no_std` by default — any change to either must
also be checked against the embedded and wasm targets declared in
`rust-toolchain.toml`:

```sh
cargo build -p tpt-gis-core -p tpt-gis-geom --target thumbv7em-none-eabihf
cargo build -p tpt-gis-core -p tpt-gis-geom --target wasm32-unknown-unknown
```

License/dependency check (mirrors CI, requires `cargo install cargo-deny`):

```sh
cargo deny check
```

These four checks (build+test on all OSes, no_std cross-compile, fmt, clippy, and
`cargo-deny`) are exactly what `.github/workflows/ci.yml` runs — reproduce them
locally before considering a change done.

## Architecture notes

**`no_std` discipline.** `tpt-gis-core` and `tpt-gis-geom` compile without `std` by
default (`std` is an opt-in feature flag), using `libm` for transcendental math
functions (`sin`, `cos`, `atan2`, `sqrt`, `pow`, ...) instead of the standard library's
f64 methods. When adding math to either crate, import from `libm` rather than calling
`f64` methods that require `std` (e.g. `libm::sqrt(x)` not `x.sqrt()` — but
`to_radians()`/`to_degrees()` are `const fn` on `f64` and don't need `std`, so those are
fine as-is). `tpt-gis-geom` geometries also borrow their point data (`&[Point]`) rather
than own it, so they can be built over `const`/`static` data with zero heap allocation
— this is a hard requirement driven by the drone-geofence MVP, not a style preference.

**Workspace-level lints (`Cargo.toml` `[workspace.lints]`), inherited via `[lints]
workspace = true` in every crate:** `unsafe_code = "deny"` and `clippy::all` as a
warning. Avoid `unsafe`; if it's ever truly unavoidable, it needs a `// SAFETY:`
comment and explicit discussion in the PR (see `CONTRIBUTING.md`).

**Module structure within `tpt-gis-core`:** `Crs` (`crs.rs`) is a small closed enum
(`Wgs84`, `WebMercator`, `Utm(Zone)`) rather than a general EPSG-code-driven system —
`epsg.rs` documents how this is expected to grow. `Ellipsoid` (`ellipsoid.rs`) provides
`WGS84`/`GRS80` as associated consts built from `(semi_major_axis, inverse_flattening)`.
`geodesic.rs` implements Vincenty's inverse/direct formulae; `projection/utm.rs` and
`projection/web_mercator.rs` implement forward/inverse projections. Types compose:
projections and geodesics take `&Ellipsoid` and operate on `GeoPoint`/`PlanarPoint`
from the crate root.

**Module structure within `tpt-gis-geom`:** `Point`, `LineString`, `Polygon`, `Rect`
(bbox) are the OGC Simple Features primitives; `predicates.rs` builds `point_in_ring`
(ray casting), `segments_intersect` (orientation test), and higher-level
`polygons_intersect`/`polygon_contains_polygon` on top of them, short-circuiting on
bbox overlap first. Boolean set ops (`buffer`, `union`, `difference`) are intentionally
unimplemented pending a general polygon-clipping algorithm — don't attempt a partial
implementation without discussing the algorithm choice first.

**Testing convention for math code.** New geodesic/projection code needs a test against
an independently verifiable reference — a published test vector (see the Vincenty
Flinders-Peak-to-Buninyong test in `tpt-gis-core/src/geodesic.rs`, sourced from
Vincenty's own 1975 paper) or a provable invariant of the formula (see
`point_on_central_meridian_has_false_easting` in `projection/utm.rs`) — not just a
round-trip check. Round-trip tests are still useful as a secondary check (see
`forward_then_inverse_round_trips` in the same file) but don't substitute for one of the
above.

**Licensing is enforced, not aspirational.** The project's entire value proposition
rests on being strictly MIT/Apache-2.0-compatible with zero GPL/LGPL dependencies
(`deny.toml`'s `licenses.allow` list, checked by `cargo-deny` in CI). Any new dependency
must be checked against this before adding it.

**Public API doc coverage.** `missing_docs` is a workspace-level warning (trending
toward deny per `CONTRIBUTING.md`) — every public item needs a doc comment.
