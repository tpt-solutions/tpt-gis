# tpt-gis-geom

Vector geometry primitives and topological predicates for the
[`tpt-gis`](https://crates.io/crates/tpt-gis) engine.

This crate is `no_std` by default. Geometries **borrow** their point data
(`&[Point]`) rather than owning it, so they can be built over `const`/`static`
data with zero heap allocation — the requirement that drives the embedded drone
geofence use case.

## Installation

```sh
cargo add tpt-gis-geom
```

## Example

```rust
use tpt_gis_geom::{Point, Rect};

let p = Point::new(1.0, 2.0);
let bbox = Rect::new(Point::new(0.0, 0.0), Point::new(10.0, 10.0));

assert!(bbox.contains_point(p));
```

`Point`, `LineString`, `Polygon`, `Rect` (bounding box) and the `predicates`
module (`point_in_ring`, `segments_intersect`, `polygons_intersect`,
`polygon_contains_polygon`) are all available. Boolean set operations
(`buffer`, `union`, `difference`) are not yet implemented.

## License

Licensed under either of [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0)
or [MIT license](https://opensource.org/licenses/MIT) at your option.
