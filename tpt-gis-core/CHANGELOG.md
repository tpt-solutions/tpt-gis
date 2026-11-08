# Changelog

All notable changes to this crate will be documented in this file. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

Version `0.1.0` is the first release, published to crates.io on 2026-11-09.

## [Unreleased]

## [0.1.0] - 2026-11-09

### Added
- OSGB36 and the British National Grid: `Crs::Osgb36` (EPSG:4277) and `Crs::BritishNationalGrid`
  (EPSG:27700), `Ellipsoid::AIRY_1830`, and `projection::transverse_mercator::british_national_grid`.
  The datum shift to WGS84 is the Ordnance Survey's Helmert transform (about 5 m; OSTN15 is not
  used). Checked against PROJ and the Ordnance Survey's worked example. The `Crs` enum has new
  variants, so exhaustive matches need the new arms.
- `datum::inverse_transform_geocentric` — the exact inverse of `transform_geocentric`.
- `Ellipsoid` — `WGS84` / `GRS80` reference ellipsoids, plus `from_inverse_flattening`.
- `GeoPoint` / `PlanarPoint` / `Geocentric` — angular, projected and ECEF coordinate types.
- `geodesic` — Karney inverse (distance and bearings) and direct (destination) solutions on an
  ellipsoid, ported from GeographicLib (MIT/X11).
- `geodesic::polygon_measure`: perimeter and enclosed area of a polygon on the ellipsoid, as
  GeographicLib's `PolygonArea` computes them (antimeridian and pole handling included).
- `geodesic::sample`: points at equal arc-length steps along the geodesic between two points,
  as an iterator (both end points included).
- `projection::web_mercator` — WGS84 ↔ Web Mercator (EPSG:3857), in metres.
- `projection::utm` — WGS84 ↔ UTM (forward and inverse) with automatic and explicit zone
  selection (EPSG:326xx / 327xx).
- `projection::transverse_mercator` — generic Transverse Mercator.
- `projection::lcc` — Lambert Conformal Conic; `Lambert93` (EPSG:2154) is a preset.
- `projection::albers` — Albers Equal-Area; `conus_albers` (EPSG:5070) is a preset.
- `projection::local_tangent_plane` — local East-North-Up approximation for small areas.
- `datum` — geodetic ↔ geocentric (ECEF) conversion and 7-parameter Helmert (Bursa-Wolf)
  transformation.
- `crs` / `epsg` — a curated CRS registry (`Crs::Wgs84`, `WebMercator`, `Utm`, `Lambert93`,
  `ConusAlbers`), `epsg::lookup`, and `crs::transform` between any two supported CRSs.
- Projection tests check UTM, Lambert-93, Albers and Web Mercator against PROJ.
- `no_std` support (uses [`libm`] for math) with an opt-in `std` feature.

### Changed
- `geodesic` uses Karney's algorithm in place of Vincenty's iteration. `inverse` and `direct`
  keep their signatures. Nearly antipodal pairs now solve instead of returning
  `ConvergenceFailure`. Checked against GeographicLib on 613 problems: distance to 1e-7 m,
  bearings and positions to 1e-8 degrees.
- `Ellipsoid::from_inverse_flattening` and `GeoTransform::to_gdal` are no longer `const fn`
  (Rust 1.81 does not allow float arithmetic in `const fn`).

### Removed
- The unused `serde` dependency and feature.

### Fixed
- Geodesic inputs are validated (non-finite or out-of-range coordinates return an error), and
  poles are handled.
- The local tangent plane uses WGS84 radii.

[`libm`]: https://crates.io/crates/libm
