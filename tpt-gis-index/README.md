# tpt-gis-index

Spatial indexing — R-Trees, Quadtrees, and H3 / S2 hexagonal grid integration — for
the [`tpt-gis`](https://github.com/tpt-solutions/tpt-gis) engine.

## Why

Most geospatial work is "find the things near this point" or "which polygons contain
these points". A flat linear scan is fine for a handful of features and hopeless for a
million. `tpt-gis-index` provides the data structures that turn those queries into
logarithmic-time lookups, plus discrete global grid systems (H3, S2) for
binning/aggregation.

H3 support wraps [`h3o`](https://crates.io/crates/h3o), a mature pure-Rust
reimplementation of Uber's H3 (BSD-3-Clause, zero FFI). The S2 module is a
from-scratch, pure-Rust implementation using a Z-order (Morton) cell scheme — **not**
bit-compatible with Google's Hilbert-curve S2; treat its tokens as internal to this
crate unless/until a true Hilbert mapping is added (see the `s2` module docs).

## Installation

```sh
cargo add tpt-gis-index
```

## Modules

- **`rtree`** — Guttman R-Tree with one-at-a-time `insert` and Sort-Tile-Recursive
  `bulk_load`; bbox-filtered `query`.
- **`quadtree`** — region quadtree over a fixed universe, subdividing by quadrant.
- **`h3`** — H3 cell lookup, boundary, center, `k_ring`, and parsing, via `h3o`.
- **`s2`** — Z-order S2-style `S2CellId` with parent/child navigation and hex tokens.
- **`spatial_join`** — R-Tree-accelerated points-in-polygons join (in-memory,
  streaming, and batched variants).

## Examples

### R-Tree: bulk load and query

```rust
use tpt_gis_geom::{Point, Rect};
use tpt_gis_index::RTree;

let entries = vec![
    (Rect::new(Point::new(0.0, 0.0), Point::new(1.0, 1.0)), "a"),
    (Rect::new(Point::new(5.0, 5.0), Point::new(6.0, 6.0)), "b"),
    (Rect::new(Point::new(10.0, 10.0), Point::new(11.0, 11.0)), "c"),
];
let tree = RTree::bulk_load(entries);
let hits = tree.query(&Rect::new(Point::new(-1.0, -1.0), Point::new(2.0, 2.0)));
assert_eq!(hits, vec![&"a"]);
```

`RTree::bulk_load` (STR) is preferred when the whole dataset is available up front;
`RTree::insert` is for incremental growth.

### H3 hexagonal grid

```rust
use tpt_gis_index::h3;

let cell = h3::cell_at(37.7749, -122.4194, 9).unwrap();
println!("cell = {cell} (resolution {})", u8::from(cell.resolution()));

let center = h3::cell_center(cell);
println!("center = ({}, {})", center.x, center.y);

// All cells within 2 grid steps — handy for neighbourhood aggregation.
let neighbours = h3::k_ring(cell, 2);
println!("k=2 ring has {} cells", neighbours.len());
```

### S2 cell tokens

```rust
use tpt_gis_index::s2::S2CellId;

let id = S2CellId::from_lat_lon_degrees(40.743, -74.0008);
println!("token = {}", id.to_token());           // 16 hex digits
let coarse = id.parent(10);
println!("parent token = {}", coarse.to_token());
assert_eq!(S2CellId::from_token(&coarse.to_token()), Some(coarse));
```

### Spatial join (points in polygons)

```rust
use tpt_gis_geom::Point;
use tpt_gis_index::spatial_join::{spatial_join, IndexedPoint, IndexedPolygon, SpatialJoinConfig};
use tpt_gis_io::geometry::{Point as IoPoint, Polygon as IoPolygon};

let points = vec![
    IndexedPoint { point: Point::new(1.0, 1.0), payload: 0usize },
    IndexedPoint { point: Point::new(15.0, 15.0), payload: 1usize },
];
let polygons = vec![IndexedPolygon {
    polygon: IoPolygon::from_exterior(vec![
        IoPoint::new(0.0, 0.0), IoPoint::new(10.0, 0.0),
        IoPoint::new(10.0, 10.0), IoPoint::new(0.0, 10.0), IoPoint::new(0.0, 0.0),
    ]),
    payload: 100usize,
}];

let results = spatial_join(&points, &polygons, SpatialJoinConfig::default());
// One point (payload 0) falls inside the polygon (payload 100).
assert_eq!(results.len(), 1);
assert_eq!(results[0].point_payload, 0);
assert_eq!(results[0].polygon_payload, 100);
```

> The `spatial_join` API works in `f64` planar space; project lon/lat to meters
> first (e.g. with `tpt-gis-core`) for geodesic-correct results.

## License

Licensed under either of [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0)
or [MIT license](https://opensource.org/licenses/MIT) at your option.
