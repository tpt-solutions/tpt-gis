# Changelog

All notable changes to the `tpt-gis-py` Python package will be documented in this file. The
format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

No version has been published to PyPI yet; everything is under **Unreleased** and the first
release will be `0.1.0`.

## [Unreleased]

### Added
- `Geometry` — parse and serialise geometries as WKT, WKB and GeoJSON (`from_wkt`, `from_wkb`,
  `from_geojson`, `to_wkt`, `to_wkb`, `to_geojson`), with `geometry_type`, `bounds`, `area`,
  `length`, `contains_point`, and `__geo_interface__`.
- `Point` and `GeoPoint` — value types with equality, hashing and pickling.
- `InverseResult` — distance and bearings from `geodesic_inverse`.
- `geodesic_distance`, `geodesic_inverse`, `geodesic_destination` on the WGS84 and GRS80
  ellipsoids (Karney's algorithm).
- `read_shapefile_geometry` and `read_shapefile_attributes`.
- `read_zipped_shapefile` — reads a shapefile from a `.zip` archive, returning geometries and
  attributes; a missing `.dbf` gives no attributes, and an archive without a `.shp` is a
  `ValueError`.
- `coordinates(geometry)` — every vertex as an `(n, 2)` NumPy array (NumPy is imported lazily).
- Exception hierarchy: `TptGisError` (a `ValueError` subclass), `GeometryParseError` and
  `GeodesyError`.
- Type stubs and `py.typed` under the `tpt_gis_py` package; the native module is
  `tpt_gis_py._core`.

### Changed
- The shapefile readers release the GIL while reading (`allow_threads`).
- Geodesy uses Karney's algorithm.

### Fixed
- Invalid geodesic inputs raise `GeodesyError` instead of failing inside the bindings.
