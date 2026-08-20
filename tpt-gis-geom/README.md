# tpt-gis-geom

Vector geometry primitives and topological predicates for the
[`tpt-gis`](https://github.com/tpt-solutions/tpt-gis) engine.

This crate is `no_std` by default. Geometries **borrow** their point data
(`&[Point]`) rather than owning it, so they can be built over `const`/`static`
data with zero heap allocation — the requirement that drives the embedded drone
geofence use case.

## Why

The geometry layer is deliberately small and allocation-free. It gives you the OGC
Simple Features primitives (`Point`, `LineString`, `Polygon`), bounding boxes, and
the predicates (containment, intersection, distance) that everything else — spatial
indexes, file parsers, the geofence engine — builds on.

## Installation

```sh
cargo add tpt-gis-geom
```

## Features

| Feature | Default | Description                                          |
| ------- | ------- | ---------------------------------------------------- |
| `std`   | off     | Opt into the standard library where helpful.         |
| `alloc` | off     | Enable `alloc`-only APIs (owned scratch buffers).    |
| `serde` | off     | Derive `Serialize`/`Deserialize` on the primitives.  |

## Modules

- **`point`** — `Point`, a planar `(x, y)` coordinate (unitless; the CRS decides
  what the numbers mean).
- **`line_string`** — `LineString`, an ordered borrowed path with `length()` and
  segment iteration.
- **`polygon`** — `Polygon`, an exterior ring with zero or more interior rings
  (holes); `contains_point` and `nearest_boundary_point`.
- **`bbox`** — `Rect`, an axis-aligned bounding box with `contains_point`,
  `intersects`, `union`, `from_points`.
- **`distance`** — planar point/segment/line distances (Cartesian; for ellipsoidal
  distance between lon/lat use `tpt-gis-core`'s `geodesic`).
- **`predicates`** — `point_in_ring`, `segments_intersect`,
  `linestrings_intersect`, `polygons_intersect`, `polygon_contains_polygon`
  (EPSILON-tolerant).

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

let line = LineString::new(&[
    Point::new(0.0, 0.0),
    Point::new(3.0, 4.0),
    Point::new(3.0, 0.0),
]);
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

let a = Polygon::from_exterior(&[
    Point::new(0.0, 0.0), Point::new(10.0, 0.0),
    Point::new(10.0, 10.0), Point::new(0.0, 10.0), Point::new(0.0, 0.0),
]);
let b = Polygon::from_exterior(&[
    Point::new(5.0, 5.0), Point::new(15.0, 5.0),
    Point::new(15.0, 15.0), Point::new(5.0, 15.0), Point::new(5.0, 5.0),
]);
assert!(predicates::polygons_intersect(&a, &b));
assert!(predicates::polygon_contains_polygon(&a, &Polygon::from_exterior(&[
    Point::new(2.0, 2.0), Point::new(8.0, 2.0),
    Point::new(8.0, 8.0), Point::new(2.0, 8.0), Point::new(2.0, 2.0),
])));
```

> **Not yet implemented:** Boolean set operations (`buffer`, `union`,
> `difference`) require a general polygon-clipping algorithm and are tracked as
> follow-up work.

## `no_std`

`Point`, `LineString`, `Polygon`, `Rect` and all predicates are `no_std`-clean and
heap-allocation-free. The `drone-geofence-demo` in the workspace runs the geofence
check directly on a `thumbv7em-none-eabihf` target with `--no-default-features`.

## License

Licensed under either of [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0)
or [MIT license](https://opensource.org/licenses/MIT) at your option.
