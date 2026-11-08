# tpt-gis

A pure Rust, `no_std`-capable, planetary-scale GIS engine — a zero-FFI replacement
for the GDAL/PROJ/GEOS stack.

`tpt-gis` is a convenience **facade** that re-exports the five engine crates behind
feature flags, so most users depend on this single crate rather than the individual
ones.

> **Status:** not yet published to crates.io. Depend on it from git or a path (see
> below).

## Installation

```toml
[dependencies]
tpt-gis = { git = "https://github.com/tpt-solutions/tpt-gis" }
```

All five engine crates are enabled by default. Each can be toggled individually;
`io` and `index` imply `geom`, and `http` implies `raster` plus the async HTTP
Range-request reader.

Minimum supported Rust version: 1.87.

## Feature flags

| Feature  | Default | Enables                                                   |
| -------- | ------- | --------------------------------------------------------- |
| `std`    | on      | Host `std` math intrinsics for `core` and `geom`.         |
| `core`   | on      | CRS definitions, datum transforms, geodesic math.         |
| `geom`   | on      | Vector geometry, predicates, overlay and buffer.          |
| `io`     | on      | GeoJSON, WKB, WKT, Shapefile and GeoPackage parsers and writers. |
| `index`  | on      | R-trees, quadtrees, H3 and S2 indexes, spatial joins.     |
| `raster` | on      | Grid data, map algebra, GeoTIFF and COG read/write.       |
| `http`   | off     | Async HTTP Range-request streaming reader for remote COGs. |

## Example

```rust
use tpt_gis::core::{Ellipsoid, GeoPoint, geodesic};
use tpt_gis::geom::Point;

let a = GeoPoint::new(-37.8136, 144.9631);
let b = GeoPoint::new(-33.8688, 151.2093);

let distance_m = geodesic::inverse(&Ellipsoid::WGS84, a, b).unwrap().distance_m;
println!("distance = {distance_m} m");

let p = Point::new(1.0, 2.0);
println!("point = ({}, {})", p.x, p.y);
```

The [`prelude`](src/prelude.rs) module imports the common types in one line.

## Crates

- [`tpt-gis-core`](https://github.com/tpt-solutions/tpt-gis/tree/master/tpt-gis-core) — CRS definitions, projections, datum transforms, geodesic math (`no_std`).
- [`tpt-gis-geom`](https://github.com/tpt-solutions/tpt-gis/tree/master/tpt-gis-geom) — vector geometry, predicates, overlay and buffer (`no_std`).
- [`tpt-gis-io`](https://github.com/tpt-solutions/tpt-gis/tree/master/tpt-gis-io) — GeoJSON, WKB, WKT, Shapefile and GeoPackage parsers and writers.
- [`tpt-gis-index`](https://github.com/tpt-solutions/tpt-gis/tree/master/tpt-gis-index) — R-trees, quadtrees, H3 and S2, spatial joins.
- [`tpt-gis-raster`](https://github.com/tpt-solutions/tpt-gis/tree/master/tpt-gis-raster) — grid data, map algebra, GeoTIFF and COG read/write.

See the [workspace README](https://github.com/tpt-solutions/tpt-gis) for the design
rationale, the examples, the CLI, and the Python bindings.

## License

Licensed under either of [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0)
or [MIT license](https://opensource.org/licenses/MIT) at your option.
