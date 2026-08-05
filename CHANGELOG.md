# Changelog

All notable changes to this project will be documented in this file. The format
is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [0.1.0] - 2025-08-05

### Added
- `tpt-gis-core`: CRS definitions (WGS84, Web Mercator, UTM), geodesic math (Vincenty), datum transformations, local tangent-plane projection
- `tpt-gis-geom`: `no_std` OGC Simple Features primitives (`Point`, `LineString`, `Polygon`), bounding boxes, point-in-polygon, intersects/contains/distance predicates
- `tpt-gis-io`: GeoJSON, WKB, WKT, and Shapefile readers/writers
- `tpt-gis-index`: R-Tree (bulk + incremental), Quadtree, H3 hexagonal grid integration, spatial-join engine
- `drone-geofence-demo`: MVP 2 real-time drone geofence demo with escape-vector calculation
- `spatial-join-cli`: MVP 1 CLI with `spatial-join` and `reproject` subcommands, CSV/GeoJSON output
- Property-based tests (proptest) for geometry and geodesic invariants
- Criterion benchmarks for geofence checks and spatial join throughput
- CI for std, `no_std`, WASM, and WASI targets
- `cargo-deny` license/advisory enforcement

[0.1.0]: https://github.com/tpt-solutions/tpt-gis/commit/4bbd4a7721d0e0a394ba92cadd1c8c882f20392f
