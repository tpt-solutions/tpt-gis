# Changelog

All notable changes to this crate will be documented in this file. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

## [0.1.0] - 2025-08-05

### Added
- `Point` — an OGC Simple Features `Point` (planar `x`, `y`).
- `LineString` — an ordered, borrowed path with length and segment iteration.
- `Polygon` — exterior ring with interior holes; borrowed `(x, y)` data so it can
  live in `const`/`static` with zero heap allocation.
- `Rect` (`bbox`) — axis-aligned bounding box with `contains_point`,
  `intersects`, `union`, and `from_points`.
- `distance` — planar point/segment/line distances (Cartesian, no datum).
- `predicates` — point-in-ring, segment intersection, line/polygon intersection,
  and polygon containment tests (EPSILON-tolerant).
- `no_std` support with `std`/`alloc`/`serde` feature flags; the core primitives
  are allocation-free so they run on embedded targets.

[Unreleased]: https://github.com/tpt-solutions/tpt-gis/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/tpt-solutions/tpt-gis/releases/tag/v0.1.0
