# tpt-gis-core

Coordinate reference systems, datum transformations, and geodesic math for the
[`tpt-gis`](https://crates.io/crates/tpt-gis) engine.

This crate is `no_std` by default (it uses [`libm`] for transcendental math so it
runs on embedded targets); enable the `std` feature to use the platform's native
math intrinsics instead.

## Installation

```sh
cargo add tpt-gis-core
```

## Example

```rust
use tpt_gis_core::{Ellipsoid, GeoPoint, geodesic, projection::web_mercator};

let melbourne = GeoPoint::new(-37.8136, 144.9631);
let sydney = GeoPoint::new(-33.8688, 151.2093);

// Geodesic distance (Vincenty's inverse) on the WGS84 ellipsoid.
let result = geodesic::inverse(&Ellipsoid::WGS84, melbourne, sydney).unwrap();
println!("distance = {} m", result.distance_m);

// Web Mercator (EPSG:3857) projection to meters.
let xy = web_mercator::forward(melbourne);
println!("web mercator = ({}, {})", xy.x, xy.y);
```

## Description

`tpt-gis-core` provides:

- **CRS** — a small, closed set of coordinate reference systems (`Wgs84`,
  `WebMercator`, `Utm`).
- **Ellipsoid** — `WGS84` / `GRS80` models and datum parameters.
- **Geodesic** — Vincenty inverse/direct solutions for distance and bearing.
- **Projection** — WGS84↔Web Mercator and WGS84↔UTM (with local tangent-plane
  support).

## License

Licensed under either of [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0)
or [MIT license](https://opensource.org/licenses/MIT) at your option.
