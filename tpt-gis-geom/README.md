# tpt-gis-geom

Vector geometry primitives, topological predicates, overlay and buffering for the
[`tpt-gis`](https://github.com/tpt-solutions/tpt-gis) engine.

This crate is `no_std` by default. Geometries **borrow** their point data
(`&[Point]`) rather than owning it, so they can be built over `const`/`static`
data with zero heap allocation — the requirement that drives the embedded drone
geofence use case.

> **Status:** not yet published to crates.io. Depend on it by path or git, for example
> `tpt-gis-geom = { git = "https://github.com/tpt-solutions/tpt-gis" }`.

## Why

The geometry layer is deliberately small and allocation-free where it can be. It gives
you the OGC Simple Features primitives (`Point`, `LineString`, `Polygon`, and the
multi-geometries), bounding boxes, and the predicates (containment, intersection,
distance, the DE-9IM matrix) that everything else — spatial indexes, file parsers, the
geofence engine — builds on. Polygon overlay and buffering allocate, so they sit behind
the `alloc` feature.

## Installation

```toml
[dependencies]
tpt-gis-geom = { git = "https://github.com/tpt-solutions/tpt-gis", features = ["alloc"] }
```

## Features

| Feature | Default | Description                                                    |
| ------- | ------- | -------------------------------------------------------------- |
| `std`   | off     | Opt into the standard library where helpful.                   |
| `alloc` | off     | Enables `clip` (overlay), `buffer`, and the DE-9IM `relate`.   |

## Modules

- **`point`** — `Point`, a planar `(x, y)` coordinate (unitless; the CRS decides what the
  numbers mean).
- **`line_string`** — `LineString`, an ordered borrowed path with `length()` and segment
  iteration.
- **`polygon`** — `Polygon`, an exterior ring with zero or more interior rings (holes);
  `contains_point` and `nearest_boundary_point`.
- **`multi`** — `MultiPoint`, `MultiLineString`, `MultiPolygon`, and `GeometryRef`.
- **`bbox`** — `Rect`, an axis-aligned bounding box with `contains_point`, `intersects`,
  `union`, `from_points`.
- **`distance`** — planar point, segment and line distances (Cartesian; for ellipsoidal
  distance between lon/lat use `tpt-gis-core`'s `geodesic`).
- **`measure`**, **`validity`**, **`hull`**, **`simplify`** — ring area and centroid,
  polygon validity (`is_valid`), convex hull, and simplification.
- **`predicates`** — `point_in_ring`, `segments_intersect`, `linestrings_intersect`,
  `polygons_intersect`, `polygon_contains_polygon`, `polygons_touch`, `polygons_overlap`
  (EPSILON-tolerant).
- **`relate`** / **`de9im`** *(alloc)* — the full DE-9IM intersection matrix of two
  geometries, with `IntersectionMatrix::matches` for patterns such as `T*F**FFF*`.
- **`clip`** *(alloc)* — polygon `union`, `intersection`, `difference`, `symmetric_difference`.
- **`buffer`** *(alloc)* — `buffer_point`, `buffer_line`, `buffer_polygon`. Negative distances
  erode a polygon; holes are handled.

## Examples

### Bounding box membership

```rust
use tpt_gis_geom::{Point, Rect};

let p = Point::new(1.0, 2.0);
let bbox = Rect::new(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
assert!(bbox.contains_point(p));
```

### Polygon containment and nearest boundary

```rust
use tpt_gis_geom::{Point, Polygon};

let square = [
    Point::new(0.0, 0.0),
    Point::new(10.0, 0.0),
    Point::new(10.0, 10.0),
    Point::new(0.0, 10.0),
    Point::new(0.0, 0.0),
];
let poly = Polygon::from_exterior(&square);

assert!(poly.contains_point(Point::new(5.0, 5.0)));
assert!(!poly.contains_point(Point::new(15.0, 5.0)));

// Closest point on the boundary to (2, 5) — e.g. the escape point out of a zone.
let nearest = poly.nearest_boundary_point(Point::new(2.0, 5.0)).unwrap();
assert_eq!(nearest, Point::new(0.0, 5.0));
```

### Line length and distances

```rust
use tpt_gis_geom::{distance, LineString, Point};

let pts = [
    Point::new(0.0, 0.0),
    Point::new(3.0, 4.0),
    Point::new(3.0, 0.0),
];
let line = LineString::new(&pts);
assert_eq!(line.length(), 9.0); // 5.0 + 4.0

let d = distance::point_to_segment(
    Point::new(0.0, 5.0),
    Point::new(-10.0, 0.0),
    Point::new(10.0, 0.0),
);
assert_eq!(d, 5.0);
```

### Topological predicates

```rust
use tpt_gis_geom::{predicates, Point, Polygon};

let ring_a = [
    Point::new(0.0, 0.0), Point::new(10.0, 0.0),
    Point::new(10.0, 10.0), Point::new(0.0, 10.0), Point::new(0.0, 0.0),
];
let ring_b = [
    Point::new(5.0, 5.0), Point::new(15.0, 5.0),
    Point::new(15.0, 15.0), Point::new(5.0, 15.0), Point::new(5.0, 5.0),
];
let ring_c = [
    Point::new(2.0, 2.0), Point::new(8.0, 2.0),
    Point::new(8.0, 8.0), Point::new(2.0, 8.0), Point::new(2.0, 2.0),
];
let a = Polygon::from_exterior(&ring_a);
let b = Polygon::from_exterior(&ring_b);
assert!(predicates::polygons_intersect(&a, &b));
assert!(predicates::polygon_contains_polygon(&a, &Polygon::from_exterior(&ring_c)));
```

### Polygon overlay and buffer *(requires `alloc`)*

```rust
use tpt_gis_geom::{buffer, clip, Point, Polygon};

let ring_a = [
    Point::new(0.0, 0.0), Point::new(10.0, 0.0),
    Point::new(10.0, 10.0), Point::new(0.0, 10.0), Point::new(0.0, 0.0),
];
let ring_b = [
    Point::new(5.0, 5.0), Point::new(15.0, 5.0),
    Point::new(15.0, 15.0), Point::new(5.0, 15.0), Point::new(5.0, 5.0),
];
let a = Polygon::from_exterior(&ring_a);
let b = Polygon::from_exterior(&ring_b);

// Each result is a Vec of owned polygons.
let overlap = clip::intersection(&a, &b);
let merged = clip::union(&a, &b);
let rounded = buffer::buffer_point(Point::new(0.0, 0.0), 1.0, 16);
```

## `no_std`

`Point`, `LineString`, `Polygon`, `Rect`, the multi-geometries and the predicates are
`no_std`-clean and heap-allocation-free. CI cross-compiles this crate for
`thumbv7em-none-eabihf`, `riscv32imc-unknown-none-elf`, `wasm32-unknown-unknown` and
`wasm32-wasip1`.

## License

Licensed under either of [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0)
or [MIT license](https://opensource.org/licenses/MIT) at your option.
