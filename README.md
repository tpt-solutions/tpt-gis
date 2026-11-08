# tpt-gis

[![CI](https://github.com/tpt-solutions/tpt-gis/actions/workflows/ci.yml/badge.svg)](https://github.com/tpt-solutions/tpt-gis/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![MSRV](https://img.shields.io/badge/MSRV-1.87-blue)](https://www.rust-lang.org)

A pure Rust, planetary-scale Geographic Information Systems engine — a zero-FFI
replacement for the GDAL/PROJ/GEOS stack, with `no_std` support for edge/embedded
targets and cloud-native streaming for planetary-scale raster/vector data.

**License:** Dual [MIT](LICENSE-MIT) / [Apache-2.0](LICENSE-APACHE), at your option.
**Status:** Pre-release. Nothing is published to crates.io or PyPI yet, so the install
steps below build from this repository. See [`CHANGELOG.md`](CHANGELOG.md) for what has
changed and [`todo.md`](todo.md) for the roadmap.

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

## Crates

| Crate | What it provides | `no_std` |
| ----- | ---------------- | -------- |
| [`tpt-gis-core`](tpt-gis-core) | CRS registry, projections, Karney geodesics, datum transforms | Yes |
| [`tpt-gis-geom`](tpt-gis-geom) | OGC geometry, predicates, DE-9IM, overlay and buffer | Yes (overlay and buffer need `alloc`) |
| [`tpt-gis-io`](tpt-gis-io) | GeoJSON, WKT, WKB, Shapefile and GeoPackage readers and writers | No |
| [`tpt-gis-index`](tpt-gis-index) | R-trees, quadtrees, H3 and S2 grids, spatial joins | No |
| [`tpt-gis-raster`](tpt-gis-raster) | Grids, map algebra, resampling, GeoTIFF and COG read/write | Grid, algebra and resample only |
| [`tpt-gis`](tpt-gis) | Facade crate that re-exports the five crates behind feature flags | Via features |
| [`tpt-gis-cli`](tpt-gis-cli) | The `tptgis` command-line tool | No |
| [`bindings/tpt-gis-py`](bindings/tpt-gis-py) | Python bindings (`tpt_gis_py`) | No |

## Architecture

```
tpt-gis/
├── tpt-gis-core      # CRS definitions, datum transformations, geodesic math (no_std)
├── tpt-gis-geom      # Vector geometry (points, lines, polygons), topological ops (no_std)
├── tpt-gis-raster    # Grid data, map algebra, GeoTIFF / Cloud Optimized GeoTIFF (COG)
├── tpt-gis-io        # Parsers and writers for GeoJSON, WKB/WKT, Shapefile, GeoPackage
├── tpt-gis-index     # R-Trees, Quadtrees, H3/S2 hexagonal grid integration
├── tpt-gis           # Facade crate re-exporting the five crates behind feature flags
├── tpt-gis-cli       # `tptgis` command-line tool
└── bindings/tpt-gis-py  # Python bindings
```

See [`spec.txt`](spec.txt) for the full design document, [`todo.md`](todo.md) for
the phased task breakdown, and [`docs/`](docs) for guides.

## Quickstart (Rust)

Until a release is published, depend on the facade crate from git:

```toml
[dependencies]
tpt-gis = { git = "https://github.com/tpt-solutions/tpt-gis" }
```

Then use any of the engine crates behind feature flags. Most users only need the
`core` (geodesy) and `geom` (vector topology) features:

```rust
use tpt_gis::core::{geodesic, Ellipsoid, GeoPoint};
use tpt_gis::geom::{Point, Polygon};

// Geodesic distance between two places on the WGS84 ellipsoid (Karney).
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

## Command-line tool

```sh
cargo install --path tpt-gis-cli       # installs the `tptgis` binary

tptgis info cities.geojson
tptgis spatial-join --points points.geojson --polygons zones.geojson --output joined.geojson
tptgis convert roads.shp --to gpkg --output roads.gpkg
```

Subcommands: `spatial-join`, `reproject`, `convert`, `info`, `validate`,
`geofence-compile`, `generate-fixtures` and `completions`. See
[`tpt-gis-cli`](tpt-gis-cli/README.md) and [CLI usage](docs/cli-usage.md).

## Python bindings

`tpt-gis` ships Python bindings — [`tpt-gis-py`](bindings/tpt-gis-py), built with
[pyo3](https://pyo3.rs) and [maturin](https://github.com/PyO3/maturin). Build from source
(Python 3.8+):

```sh
cd bindings/tpt-gis-py
pip install maturin
maturin develop
```

```python
import tpt_gis_py as gis

poly = gis.Geometry.from_wkt("POLYGON ((0 0, 10 0, 10 10, 0 10, 0 0))")
assert poly.contains_point(5.0, 5.0)

london = gis.GeoPoint(51.5074, -0.1278)
paris = gis.GeoPoint(48.8566, 2.3522)
gis.geodesic_distance(london, paris)  # ≈ 343_500 m on WGS84
```

See [Python bindings](docs/python-bindings.md) for the full API. Raster/COG, the index
crate, GeoPackage and projections are not yet exposed to Python.

## Building and testing

```sh
cargo build --workspace
cargo test --workspace
```

The minimum supported Rust version is 1.87. `tpt-gis-core` and `tpt-gis-geom` are
`no_std` by default (enable the `std` feature for host-native math intrinsics). CI
cross-compiles them for `thumbv7em-none-eabihf`, `riscv32imc-unknown-none-elf` (the
ESP32-C3 target), `wasm32-unknown-unknown` and `wasm32-wasip1`.

The workspace excludes `examples/wasm-geofence-demo`, `examples/wasm-playground`, the cargo-fuzz harnesses, and the
Python binding; each has its own build steps in its README.

## Examples

- [`examples/spatial-join-cli`](examples/spatial-join-cli) — the spatial-join criterion
  benchmark host, and a thin wrapper around the same subcommands as `tptgis`.
- [`examples/drone-geofence-demo`](examples/drone-geofence-demo) — a real-time `no_std`
  drone geofence / navigation engine (breach detection and escape vector).
- [`examples/wasm-geofence-demo`](examples/wasm-geofence-demo) — a browser/WASM demo that
  reuses the drone geofence `check_position` logic in a `wasm-bindgen` + canvas harness.
- [`examples/wasm-playground`](examples/wasm-playground) — a browser playground that loads
  GeoJSON, reprojects it and joins points to polygons, all client-side. Published to GitHub
  Pages from `master`.

## Documentation

- [Getting started](docs/getting-started.md) and [cookbook](docs/cookbook.md)
- [CLI usage](docs/cli-usage.md) and [Python bindings](docs/python-bindings.md)
- [Coming from GDAL, PROJ and GEOS](docs/coming-from-gdal-proj-geos.md)
- [Benchmarks](docs/benchmarks.md)

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
