# Changelog

All notable changes to this crate will be documented in this file. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

Version `0.1.0` is the first release, published to crates.io on 2026-11-09.

## [Unreleased]

## [0.1.0] - 2026-11-09

### Added
- `Point` — an OGC Simple Features `Point` (planar `x`, `y`).
- `LineString` — an ordered, borrowed path with length and segment iteration.
- `Polygon` — exterior ring with interior holes; borrowed `(x, y)` data so it can live in
  `const`/`static` with zero heap allocation. Area and centroid.
- `MultiPoint`, `MultiLineString`, `MultiPolygon` — borrowed multi-geometries, and
  `GeometryRef` to pass any of them to `relate`.
- `Rect` (`bbox`) — axis-aligned bounding box with `contains_point`, `intersects`, `union`,
  and `from_points`.
- `distance` — planar point, segment and line distances (Cartesian, no datum).
- `predicates` — point-in-ring, segment intersection, line/line and line/polygon intersects,
  crosses and within, polygon intersection, containment, touches and overlaps (EPSILON-tolerant).
- `relate::relate` and `de9im` — the full DE-9IM intersection matrix of two geometries (points,
  multi-points, line strings, multi-line strings, polygons, multi-polygons), with
  `IntersectionMatrix::matches` for patterns such as `T*F**FFF*`. Checked against GEOS on 1200
  random pairs. Collections return `None`. The parts of a multi-polygon are assumed not to
  overlap.
- `measure` / `validity` / `hull` / `simplify` — ring signed area, polygon validity
  (`is_valid`), convex hull, and simplification.
- `clip` / `overlay` (requires `alloc`) — polygon `union`, `intersection`, `difference` and
  `symmetric_difference`.
- `buffer` (requires `alloc`) — `buffer_point`, `buffer_line` and `buffer_polygon`. Polygons with
  holes are buffered by eroding the holes. Negative distances erode.
- `no_std` support with `std` and `alloc` feature flags; the primitives and predicates are
  allocation-free and run on embedded targets.

### Changed
- Containment and intersection tests respect polygon holes.
- Nearness tolerance scales with the coordinates.
- Clip split points are ordered along their edge.

### Removed
- The unused `serde` dependency and feature.

### Fixed
- `point_in_ring` is defined for points on the boundary.
- Polygon clipping output is checked against GEOS.
