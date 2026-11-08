# Changelog

Workspace-level changes for the tpt-gis engine. Per-crate changes are in each crate's own
`CHANGELOG.md`. The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

No version has been tagged or published yet (see [`todo.md`](todo.md), Phase 4.8). Everything
below is therefore under **Unreleased**; the first release will be `0.1.0`.

## [Unreleased]

### Added
- `tpt-gis-core`: CRS registry (WGS84, Web Mercator, UTM, RGF93 / Lambert-93 EPSG:2154, NAD83 /
  Conus Albers EPSG:5070) with `epsg::lookup`; projections (Web Mercator, UTM, generic
  Transverse Mercator, Lambert Conformal Conic, Albers, local tangent plane); geodesics on the
  ellipsoid (Karney inverse/direct, polygon area and perimeter, equal-step sampling); datum
  transforms (geodetic/ECEF and 7-parameter Helmert); general `crs::transform` between supported
  CRSs.
- `tpt-gis-geom`: OGC Simple Features primitives (`Point`, `LineString`, `Polygon`, `Rect`) and
  borrowed multi-geometries; planar distances; containment and intersection predicates; the full
  DE-9IM intersection matrix; area, centroid, validity, convex hull and simplification; and, with
  `alloc`, polygon `union` / `intersection` / `difference` / `symmetric_difference` and `buffer`.
- `tpt-gis-io`: GeoJSON, WKT, WKB, Shapefile and GeoPackage readers; GeoJSON, WKT, WKB, Shapefile
  (`.shp`/`.shx`/`.dbf`) and GeoPackage writers, including a GeoPackage R-tree spatial index.
- `tpt-gis-index`: R-tree (incremental and STR bulk load, nearest-neighbour queries), quadtree,
  H3 (via `h3o`) and S2-style cell indexes with polygon coverings, and R-tree-accelerated
  points-in-polygons spatial joins.
- `tpt-gis-raster`: `Grid` / `Band` types, nodata-aware map algebra, resampling and overviews;
  GeoTIFF reading (including ZSTD, multi-band and `PixelIsPoint`) and writing; Cloud Optimized
  GeoTIFF writing and validation; async HTTP Range-request COG reader (`http` feature).
- `tpt-gis-cli` (binary `tptgis`): `spatial-join`, `reproject`, `convert`, `info`, `validate`,
  `geofence-compile`, `generate-fixtures` and `completions` subcommands.
- `tpt-gis`: facade crate re-exporting the engine crates behind feature flags, with a `prelude`.
- `tpt-gis-py`: Python bindings (pyo3 + maturin) for geometry I/O, containment, shapefile reading,
  geodesy, NumPy vertex arrays, type stubs and a `TptGisError` exception hierarchy.
- Examples: `spatial-join-cli` (benchmark host), `drone-geofence-demo` (`no_std` geofence) and
  `wasm-geofence-demo` (browser harness).
- CI: MSRV job, rustdoc with warnings as errors, feature-powerset checks, `no_std` cross-compiles,
  line coverage, scheduled advisory and fuzz runs.

### Changed
- Minimum supported Rust version is 1.87 (raised for the ZSTD decoder, `ruzstd` 0.9.1).
- Geodesic calculations use Karney's algorithm (a GeographicLib port) in place of Vincenty's
  iteration.
- Dependencies `rand` and `csv` were replaced with in-crate modules.

### Removed
- The unused `serde` dependency and feature from `tpt-gis-core` and `tpt-gis-geom`.

### Fixed
- Geodesic inputs are validated, poles are handled, and the local tangent plane uses WGS84 radii.
- Geometry containment and intersection respect polygon holes; `point_in_ring` is defined for
  points on the boundary; nearness tolerances scale with the coordinates.
- GeoPackage: `INTEGER PRIMARY KEY` records keep their NULL slot, the `gpkg_extensions` table has
  its UNIQUE index, and the readers are hardened against malformed input.
- Shapefile: null shapes keep their position; `.cpg` encodings are honoured.
- GeoTIFF / COG: predictors and short strips are decoded correctly, `PixelIsPoint` georeferencing
  is honoured, LZW output is bounded, and TIFF and HTTP readers are hardened. Checked against GDAL.
- Index: S2 ids are validated, quadtree depth is bounded, and the R-tree was extended.
- Reprojection and join fixes in the CLI.

[Unreleased]: https://github.com/tpt-solutions/tpt-gis/commits/master
