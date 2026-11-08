# Changelog

All notable changes to this crate will be documented in this file. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

Version `0.1.0` is the first release, published to crates.io on 2026-11-09.

## [Unreleased]

## [0.1.0] - 2026-11-09

### Added
- `tptgis` binary with these subcommands:
  - `spatial-join` — points-in-polygons join from GeoJSON, output as GeoJSON or CSV, with
    `--incremental` for R-tree insertion instead of bulk loading.
  - `reproject` — reproject a GeoJSON file from WGS84 to Web Mercator, a UTM zone (`utm-18n`), or
    a UTM EPSG code (32601–32660, 32701–32760).
  - `convert` — convert between GeoJSON, WKT, WKB, Shapefile and GeoPackage. Reads stdin with
    `--input -`; `--spatial-index` writes the GeoPackage R-tree.
  - `info` — JSON summary of a vector file: feature count, geometry types, bounds, and property
    names. Reports a shapefile's projection.
  - `validate` — checks vector geometry and COG layout; prints a JSON report and exits non-zero on
    any problem.
  - `geofence-compile` — compiles GeoJSON polygons into a `no_std` Rust module of const data.
  - `generate-fixtures` — synthetic GeoJSON test data.
  - `completions` — shell completion scripts.
- Shapefile output writes a `.prj` sidecar, and shapefile attributes carry through `convert`,
  `info` and `validate`.
- Integration tests in `tests/cli.rs`; `run_with_args` is testable and does not exit the process.
- Global `-v` / `--verbose` prints progress to standard error.
- `info` reads standard input with `--from <format>`.
- `spatial-join` reads either `--points` or `--polygons` from standard input (`-`).
- `spatial-join --format csv` writes a header row (`x,y` and the point property names), and
  string values are no longer wrapped in JSON quotes.
- Errors from `info`, `convert`, `reproject`, `geofence-compile` and `spatial-join` name the
  file they came from.

### Changed
- Dependencies `rand` and `csv` were replaced with in-crate modules.
