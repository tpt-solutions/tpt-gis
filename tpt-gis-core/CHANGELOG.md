# Changelog

All notable changes to this crate will be documented in this file. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

## [0.1.0] - 2025-08-05

### Added
- `Ellipsoid` — `WGS84` / `GRS80` reference ellipsoids and datum parameters.
- `GeoPoint` / `PlanarPoint` — angular and projected coordinate types.
- `geodesic` — Vincenty inverse (distance + bearings) and direct (destination)
  solutions on an ellipsoid.
- `projection::web_mercator` — WGS84 ↔ Web Mercator (EPSG:3857), meters.
- `projection::utm` — WGS84 ↔ UTM (forward/inverse) with automatic and explicit
  zone selection (EPSG:326xx / 327xx).
- `projection::local_tangent_plane` — local East-North-Up approximation for
  small-area meter-space geometry.
- `datum` — geodetic ↔ geocentric (ECEF) conversion and 7-parameter Helmert
  (Bursa-Wolf) datum transformation.
- `crs` / `epsg` — a small, curated CRS registry (WGS84, Web Mercator, UTM).
- `no_std` support (uses [`libm`] for math) with an opt-in `std` feature and
  optional `serde` derive.

[Unreleased]: https://github.com/tpt-solutions/tpt-gis/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/tpt-solutions/tpt-gis/releases/tag/v0.1.0
