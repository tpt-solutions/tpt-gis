# Changelog

All notable changes to this crate will be documented in this file. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

No version has been published yet; everything is under **Unreleased** and the first
release will be `0.1.0`.

## [Unreleased]

### Added
- `prelude` module with one-line imports of the common types from each engine crate.
- Facade crate re-exporting the five engine crates (`core`, `geom`, `io`, `index`, `raster`)
  behind feature flags, so most users depend on this single crate.
- Feature flags: `std`, `core`, `geom`, `io`, `index`, `raster`, `http` (the first five enabled
  by default; `io` and `index` imply `geom`, and `http` implies `raster` plus the async HTTP COG
  reader).

### Changed
- Minimum supported Rust version is 1.87.
