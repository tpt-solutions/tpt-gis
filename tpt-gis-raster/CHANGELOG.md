# Changelog

All notable changes to this crate will be documented in this file. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

No version has been published yet; everything is under **Unreleased** and the first
release will be `0.1.0`.

## [Unreleased]

### Added
- `Grid<T>` — dense row-major cell array over any `CellType` (`u8`/`u16`/`i16`/`u32`/`i32`/
  `f32`/`f64`); `no_std` + `alloc`.
- `Band<T>` (alias `Raster<T>`) and `GeoTransform` — georeferenced band with CRS, nodata, and
  metrics (`min_max`, `mean`, `valid_count`, `sample_world`). `AnyBand` is the runtime-typed band
  returned by file readers.
- `algebra` — nodata-aware local (add, subtract, multiply, divide, scale, offset, clamp,
  reclassify) and focal (mean, min, max, sum, range, stddev, median, kernel) operations.
- `resample` — nearest, bilinear and average resampling, `resample_to`, and `downsample` for
  overview building.
- `geotiff` — from-scratch (Geo)TIFF reader (`std`), with ZSTD-compressed files and multi-band
  reading (`read_band`, `GeoTiff::read_image_band`, `GeoTiff::band_count`).
- `geotiff::write` and `geotiff::write_bands` — uncompressed single-strip GeoTIFF writing, single
  and multi-band.
- `geotiff::write_cog` and `CogOptions` — Cloud Optimized GeoTIFF writing with tiles (16 to 4096
  pixels), optional DEFLATE, and chained overview levels from 2×2 means that skip nodata.
- `cog` — COG layout validator (`validate`, `CogReport`).
- `http` feature — async Range-request COG reader (`std` + `reqwest` over rustls).

### Changed
- `cog::validate` also reads overviews chained after the full-resolution IFD, as GDAL writes
  them, in addition to SubIFDs.
- Depends on `ruzstd` 0.9.1 for ZSTD decoding; the minimum supported Rust version is 1.87.

### Fixed
- Predictors and short strips decode correctly; the TIFF and HTTP readers are hardened.
- `PixelIsPoint` georeferencing is honoured.
- LZW decoding output is bounded.
- GeoTIFF output is checked against GDAL.
