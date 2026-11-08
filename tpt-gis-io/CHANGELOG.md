# Changelog

All notable changes to this crate will be documented in this file. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

No version has been published yet; everything is under **Unreleased** and the first
release will be `0.1.0`.

## [Unreleased]

### Added
- `geometry` — the shared owned `Geometry` / `Polygon` types produced and consumed by every
  reader and writer.
- `geojson` — GeoJSON `Feature` / `FeatureCollection` reader and writer (RFC 7946), with bounds
  on nesting depth and rejection of non-finite coordinates. Bare Geometry JSON is accepted.
- `wkb` — Well-Known Binary reader and writer (big- and little-endian), with allocation bounds
  and malformed-input guards. Accepts ISO Z/M and EWKB forms.
- `wkt` — Well-Known Text reader and writer (recursive-descent), case-insensitive keywords and
  flexible whitespace. Reads dimension tags (`Z`, `M`, `ZM`) and `SRID=`.
- `shapefile` — `.shp` / `.dbf` reader (`read_shp`, `read_shp_records`, streaming
  `ShpGeometryIter`, `read_dbf` / `read_dbf_with`). Attributes are decoded with the `.cpg`
  encoding. Z and M shape types are read as 2D.
- `shapefile_writer` — writes `.shp`, `.shx` and `.dbf` (`write_shapefile`, `write_dbf`). Rejects
  mixed shape types and geometry collections.
- `geopackage` — pure-Rust, zero-FFI GeoPackage (`.gpkg`) reader and writer, reusing the WKB
  codec (no `libsqlite3-sys`).
  - Writer: tables with interior pages and overflow records, and `WriteOptions::spatial_index`,
    which writes the `gpkg_rtree_index` extension. Adding that field is a breaking change for
    struct literals.
  - Reader: feature and metadata tables created `WITHOUT ROWID` are read from their primary-key
    index b-tree. Checked against a file written by Python's `sqlite3`.
- Host-native crate (`std`): parsing variable-sized data inherently needs heap allocation.

### Changed
- Shapefile support is no longer read-only: the writer is part of the crate.

### Fixed
- GeoPackage writer: the `INTEGER PRIMARY KEY` id column's NULL slot is written in the record. The
  reader had the same assumption, so round trips hid the bug from the crate's own tests.
- GeoPackage writer: the `UNIQUE` constraint on `gpkg_extensions` has its automatic index, so the
  file opens without a "malformed" error in other readers.
- GeoPackage and SQLite readers are hardened against malformed input.
- Shapefile: null shapes keep their position in the output.
