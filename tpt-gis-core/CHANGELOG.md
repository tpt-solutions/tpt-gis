# Changelog

All notable changes to this crate will be documented in this file. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

### Added
- OSGB36 and the British National Grid: `Crs::Osgb36` (EPSG:4277) and `Crs::BritishNationalGrid`
  (EPSG:27700), `Ellipsoid::AIRY_1830`, and `projection::transverse_mercator::british_national_grid`.
  The datum shift to WGS84 is the Ordnance Survey's Helmert transform (about 5 m; OSTN15 is not
  used). Checked against PROJ and the Ordnance Survey's worked example. The `Crs` enum has new
  variants, so exhaustive matches need the new arms.
- `datum::inverse_transform_geocentric`, the exact inverse of `transform_geocentric`.
- `geodesic::polygon_measure`: perimeter and enclosed area of a polygon on the ellipsoid, as
  GeographicLib's `PolygonArea` computes them (antimeridian and pole handling included).
- `geodesic::sample`: points at equal arc-length steps along the geodesic between two points,
  as an iterator (both end points included).

### Changed
- `geodesic` now uses Karney's algorithm (a port of GeographicLib, MIT/X11) in place of
  Vincenty's iteration. `inverse` and `direct` keep their signatures. Nearly antipodal
  pairs now solve instead of returning `ConvergenceFailure`. Checked against GeographicLib
  on 613 problems: distance to 1e-7 m, bearings and positions to 1e-8 degrees.
- `Ellipsoid::from_inverse_flattening` and `GeoTransform::to_gdal` are no longer `const fn`
  (Rust 1.81 does not allow float arithmetic in `const fn`).

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
