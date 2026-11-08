# Changelog

All notable changes to this crate will be documented in this file. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

No version has been published yet; everything is under **Unreleased** and the first
release will be `0.1.0`.

## [Unreleased]

### Added
- `RTree` — Guttman R-tree with incremental `insert`, `remove`, and Sort-Tile-Recursive
  `bulk_load`; bbox `query`, `nearest` and `nearest_iter` for nearest-neighbour search.
- `Quadtree` — region quadtree over a fixed universe, subdividing by quadrant, with bounded
  depth.
- `h3` — H3 hexagonal grid integration via [`h3o`] (cell lookup, boundary, centre, k-ring,
  parsing) and `polygon_cells`, the H3 cells whose centres fall inside a polygon with holes.
- `s2` — Z-order (Morton) S2-style cell ids with parent/child navigation, hex token
  round-tripping, id validation, and `polygon_cells`, the S2 covering of a polygon at a given
  level, with holes honoured.
- `spatial_join` — R-tree-accelerated points-in-polygons join (in-memory, streaming, and batched
  variants), plus polygon-intersection and nearest-polygon joins and `point_polygon_distance`.
- `no_std`-incompatible (relies on `tpt-gis-io`); builds for host targets only.

### Fixed
- S2 ids are validated, and quadtree depth is bounded.

[`h3o`]: https://crates.io/crates/h3o
