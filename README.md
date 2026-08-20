# tpt-gis

[![CI](https://github.com/tpt-solutions/tpt-gis/actions/workflows/ci.yml/badge.svg)](https://github.com/tpt-solutions/tpt-gis/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![MSRV](https://img.shields.io/badge/MSRV-1.81-blue)](https://www.rust-lang.org)

A pure Rust, planetary-scale Geographic Information Systems engine — a zero-FFI
replacement for the GDAL/PROJ/GEOS stack, with `no_std` support for edge/embedded
targets and cloud-native streaming for planetary-scale raster/vector data.

**License:** Dual [MIT](LICENSE-MIT) / [Apache-2.0](LICENSE-APACHE), at your option.
**Status:** Active development — see [`todo.md`](todo.md) for the full roadmap.

## Why

GDAL and PROJ are the backbone of global mapping, autonomous vehicles, and climate
science — and a 30-year-old C++ monolith with a build system that is a rite of
passage of pain. Integrating them into modern web backends, mobile apps, or edge
devices means wrestling with FFI and a dependency chain most build systems were
never designed for.

`tpt-gis` rewrites the core mathematical projections, geometry engines, and data
formats from scratch in safe, `no_std`-capable Rust: zero-FFI coordinate
transformations, high-performance spatial indexing, and modern cloud-native
raster/vector parsing.

## Architecture

```
tpt-gis/
├── tpt-gis-core      # CRS definitions, datum transformations, geodesic math (no_std)
├── tpt-gis-geom      # Vector geometry (points, lines, polygons), topological ops (no_std)
├── tpt-gis-raster    # Grid data, map algebra, Cloud Optimized GeoTIFF (COG) parsing
├── tpt-gis-io        # Parsers/writers for GeoJSON, WKB/WKT, Shapefile, GeoPackage
├── tpt-gis-index     # R-Trees, Quadtrees, H3/S2 hexagonal grid integration
└── tpt-gis           # Facade crate re-exporting the five crates behind feature flags
```

See [`spec.txt`](spec.txt) for the full design document, [`todo.md`](todo.md) for
the phased task breakdown, and [`docs/`](docs/) for usage guides.

## Quickstart

Add the facade crate to your `Cargo.toml`:

```toml
[dependencies]
tpt-gis = "0.1"
```

Then use any of the engine crates behind feature flags. Most users only need the
`core` (geodesy) and `geom` (vector topology) features:

```rust
use tpt_gis::core::{geodesic, Ellipsoid, GeoPoint};
use tpt_gis::geom::{Point, Polygon};

// Geodesic distance between two places on the WGS84 ellipsoid (Vincenty).
let london = GeoPoint::new(51.5074, -0.1278);
let paris = GeoPoint::new(48.8566, 2.3522);
let km = geodesic::inverse(&Ellipsoid::WGS84, london, paris).unwrap().distance_m / 1000.0;
assert!((km - 343.6).abs() < 1.0); // ≈ 343.6 km

// Point-in-polygon test (zero-allocation, borrowed rings).
let ring = [
    Point::new(0.0, 0.0),
    Point::new(0.0, 1.0),
    Point::new(1.0, 1.0),
    Point::new(1.0, 0.0),
    Point::new(0.0, 0.0),
];
let zone = Polygon::from_exterior(&ring);
assert!(zone.contains_point(Point::new(0.5, 0.5)));
```

For cloud-native raster, enable `raster` and `http` to stream a remote Cloud
Optimized GeoTIFF without downloading the whole file (see
[`tpt-gis-raster`](tpt-gis-raster)).

## Building

```sh
cargo build --workspace
cargo test --workspace
```

`tpt-gis-core` and `tpt-gis-geom` are `no_std` by default (enable the `std` feature
for host-native math intrinsics) and are verified to cross-compile for embedded
(`thumbv7em-none-eabihf`, `riscv32imc-unknown-none-elf` = ESP32-C3) and WebAssembly
(`wasm32-unknown-unknown`, `wasm32-wasip1`) targets.

## Examples

- [`examples/spatial-join-cli`](examples/spatial-join-cli) — MVP 1: a planetary-scale
  points-in-polygons spatial join engine CLI (reproject, join, GeoJSON/CSV output,
  throughput benchmark).
- [`examples/drone-geofence-demo`](examples/drone-geofence-demo) — MVP 2: a real-time
  `no_std` drone geofence / navigation engine (breach detection + escape-vector).
- [`examples/wasm-geofence-demo`](examples/wasm-geofence-demo) — a browser/WASM demo
  that reuses the drone geofence `check_position` logic in a `wasm-bindgen` + canvas
  harness.

## Crates

The workspace splits the engine into five crates plus a convenience facade crate
[`tpt-gis`](tpt-gis) that re-exports them behind feature flags — most users should
depend on `tpt-gis` rather than the individual crates. The facade also exposes a
[`prelude`](tpt-gis/src/prelude.rs) module for one-line imports of the common types.

## Contributing

This project is currently **issues-only** — bug reports and design discussion via
GitHub Issues are welcome, but code pull requests are not accepted at this time.
See [CONTRIBUTING.md](CONTRIBUTING.md). Contributions must use permissive
(MIT/Apache-2.0-compatible) licensed dependencies — no GPL/LGPL.

## License

Licensed under either of

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT license](LICENSE-MIT)

at your option. Unless you explicitly state otherwise, any contribution
intentionally submitted for inclusion in this project shall be dual licensed as
above, without any additional terms or conditions.
