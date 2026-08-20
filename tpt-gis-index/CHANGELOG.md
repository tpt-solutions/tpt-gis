# Changelog

All notable changes to this crate will be documented in this file. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

## [0.1.0] - 2025-08-05

### Added
- `RTree` — Guttman R-Tree (incremental insert + quadratic-split) and
  Sort-Tile-Recursive `bulk_load` for fast bbox queries.
- `Quadtree` — region quadtree over a fixed universe, subdividing by quadrant.
- `h3` — H3 hexagonal grid integration via [`h3o`] (cell lookup, boundary,
  center, k-ring, parsing).
- `s2` — from-scratch Z-order (Morton) S2-style cell ids with parent/child
  navigation and hex token round-tripping.
- `spatial_join` — R-Tree-accelerated points-in-polygons join (in-memory,
  streaming, and batched variants).
- `no_std`-incompatible (relies on `tpt-gis-io`); builds for host targets only.

[`h3o`]: https://crates.io/crates/h3o
[Unreleased]: https://github.com/tpt-solutions/tpt-gis/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/tpt-solutions/tpt-gis/releases/tag/v0.1.0
