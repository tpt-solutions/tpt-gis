# Changelog

All notable changes to this crate will be documented in this file. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

## [0.1.0] - 2025-08-05

### Added
- Facade crate re-exporting the five engine crates (`core`, `geom`, `io`, `index`,
  `raster`) behind feature flags, so most users depend on this single crate.
- Feature flags: `std`, `core`, `geom`, `io`, `index`, `raster`, `http` (the
  first five enabled by default; `io`/`index` imply `geom`, `http` implies
  `raster` + the async HTTP COG reader).

[Unreleased]: https://github.com/tpt-solutions/tpt-gis/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/tpt-solutions/tpt-gis/releases/tag/v0.1.0
