# tpt-gis-index

Spatial indexing — R-Trees, Quadtrees, and H3 / S2 hexagonal grid integration —
for the [`tpt-gis`](https://crates.io/crates/tpt-gis) engine.

H3 support wraps [`h3o`](https://crates.io/crates/h3o), a mature pure-Rust
reimplementation of Uber's H3 (BSD-3-Clause, zero FFI). The S2 module is a
from-scratch, pure-Rust implementation that produces cell ids / tokens
interoperable with Google's S2.

## Installation

```sh
cargo add tpt-gis-index
```

## Example

```rust
use tpt_gis_geom::{Point, Rect};
use tpt_gis_index::RTree;

let mut tree = RTree::new();
tree.insert(Rect::new(Point::new(0.0, 0.0), Point::new(1.0, 1.0)), "a");
tree.insert(Rect::new(Point::new(5.0, 5.0), Point::new(6.0, 6.0)), "b");

let region = Rect::new(Point::new(-1.0, -1.0), Point::new(2.0, 2.0));
let hits = tree.query(&region);
assert_eq!(hits.len(), 1);
```

`RTree::bulk_load` (Sort-Tile-Recursive) builds a higher-quality tree when the
whole dataset is available up front; `Quadtree`, `h3`, `s2`, and `spatial_join`
round out the module set.

## License

Licensed under either of [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0)
or [MIT license](https://opensource.org/licenses/MIT) at your option.
