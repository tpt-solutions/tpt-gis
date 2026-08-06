# tpt-gis

A pure Rust, `no_std`-capable, planetary-scale GIS engine — a zero-FFI
replacement for the GDAL/PROJ/GEOS stack.

`tpt-gis` is a convenience **facade** that re-exports the five engine crates
behind feature flags, so most users depend on this single crate rather than the
individual ones.

## Installation

```sh
cargo add tpt-gis
```

All five engine crates are enabled by default. Each can be toggled individually;
`io` / `index` imply `geom`, and `http` implies `raster` plus the async HTTP
Range-request reader.

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

## Crates

- `tpt-gis-core` — CRS definitions, datum transforms, geodesic math (`no_std`).
- `tpt-gis-geom` — vector geometry + topological predicates (`no_std`).
- `tpt-gis-io` — GeoJSON / WKB / WKT / Shapefile / GeoPackage parsers + writers.
- `tpt-gis-index` — R-Trees, Quadtrees, H3 / S2.
- `tpt-gis-raster` — grid data, map algebra, COG / GeoTIFF parsing.

See the [workspace README](https://github.com/tpt-solutions/tpt-gis) for the
full design rationale and examples.

## License

Licensed under either of [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0)
or [MIT license](https://opensource.org/licenses/MIT) at your option.
