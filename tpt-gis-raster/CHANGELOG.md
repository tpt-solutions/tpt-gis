# Changelog

All notable changes to this crate will be documented in this file. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

## [0.1.0] - 2025-08-05

### Added
- `Grid<T>` — dense row-major cell array over any `CellType` (`u8`/`u16`/`i16`/
  `u32`/`i32`/`f32`/`f64`); `no_std` + `alloc`.
- `Band<T>` (alias `Raster<T>`) + `GeoTransform` — georeferenced band with CRS /
  nodata and metrics (`min_max`, `mean`, `valid_count`, `sample_world`).
- `AnyBand` — runtime-typed band returned by file readers.
- `algebra` — nodata-aware local (add/sub/mul/div/scale/offset/clamp/reclassify)
  and focal (mean/min/max/sum/range/stddev/median/kernel) operations.
- `resample` — nearest / bilinear / average resampling, `resample_to`, and
  `downsample` overview building.
- `geotiff` / `cog` — from-scratch (Geo)TIFF/COG reader and COG layout validator
  (`std`); `http` feature adds an async Range-request COG reader (`std` + reqwest
  over rustls).

[Unreleased]: https://github.com/tpt-solutions/tpt-gis/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/tpt-solutions/tpt-gis/releases/tag/v0.1.0
