# Changelog

All notable changes to this crate will be documented in this file. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

## [0.1.0] - 2025-08-05

### Added
- Shared owned `Geometry` / `Polygon` representation (`geometry`) bridging
  `tpt-gis-geom`'s borrowed primitives and parsed file data.
- `geojson` — GeoJSON `Feature`/`FeatureCollection` reader + writer (RFC 7946),
  with bounds on nesting depth and non-finite-coordinate rejection.
- `wkb` — standard 2D ISO/OGC Well-Known Binary reader + writer (big- and
  little-endian), with allocation bounds and malformed-input guards.
- `wkt` — Well-Known Text reader + writer (recursive-descent), case-insensitive
  keywords and flexible whitespace.
- `shapefile` — read-only ESRI `.shp`/`.dbf` reader (sequential and streaming).
- `geopackage` — pure-Rust, zero-FFI GeoPackage (`.gpkg`) reader + single-leaf
  writer, reusing the WKB codec (no `libsqlite3-sys`).
- Host-native crate (`std`): parsing variable-sized data inherently needs heap
  allocation.

[Unreleased]: https://github.com/tpt-solutions/tpt-gis/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/tpt-solutions/tpt-gis/releases/tag/v0.1.0
