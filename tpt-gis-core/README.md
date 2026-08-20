# tpt-gis-core

Coordinate reference systems, datum transformations, and geodesic math for the
[`tpt-gis`](https://github.com/tpt-solutions/tpt-gis) engine.

This crate is `no_std` by default (it uses [`libm`] for transcendental math so it
runs on embedded targets); enable the `std` feature to use the platform's native
math intrinsics, and/or `serde` for (de)serialization of the geometry types.

## Why

Before you reach for a GDAL/PROJ binding, much of the day-to-day GIS math — "how far
is A from B on the Earth?", "what's the UTM easting of this coordinate?", "shift
this point from one datum to another" — is a handful of well-understood formulas.
`tpt-gis-core` implements those formulas from scratch in safe, `no_std` Rust.

## Installation

```sh
cargo add tpt-gis-core
```

## Features

| Feature  | Default | Description                                            |
| -------- | ------- | ------------------------------------------------------ |
| `std`    | off     | Use the platform's native `f64` math intrinsics.       |
| `serde`  | off     | Derive `Serialize`/`Deserialize` on `GeoPoint`, etc.   |

## Modules

- **`ellipsoid`** — `Ellipsoid` reference models (`WGS84`, `GRS80`) and their
  semi-axis / eccentricity parameters.
- **`geodesic`** — Vincenty inverse (distance + initial/final bearing) and direct
  (destination from start, bearing, distance) on an ellipsoid.
- **`projection`**
  - `web_mercator` — WGS84 ↔ Web Mercator (EPSG:3857), in meters.
  - `utm` — WGS84 ↔ UTM (forward + inverse) with `Zone` selection (EPSG:326xx/327xx).
  - `local_tangent_plane` — local East-North-Up approximation for small areas
    (correctly accounts for longitude shrinking with `cos(latitude)`).
- **`datum`** — geodetic ↔ geocentric (ECEF) conversion and 7-parameter Helmert
  (Bursa-Wolf) datum transformation.
- **`crs` / `epsg`** — a small, curated CRS registry (WGS84, Web Mercator, UTM).

## Examples

### Geodesic distance and bearing

```rust
use tpt_gis_core::{Ellipsoid, GeoPoint, geodesic};

let melbourne = GeoPoint::new(-37.8136, 144.9631);
let sydney = GeoPoint::new(-33.8688, 151.2093);

let result = geodesic::inverse(&Ellipsoid::WGS84, melbourne, sydney).unwrap();
println!("distance = {:.1} m", result.distance_m);
println!("initial bearing = {:.1}°", result.initial_bearing_deg);

// The direct problem: where do I end up 100 km away on a bearing of 45°?
let dest = geodesic::direct(&Ellipsoid::WGS84, melbourne, 45.0, 100_000.0);
println!("destination = ({:.4}, {:.4})", dest.destination.lat_deg, dest.destination.lon_deg);
```

### Projections (Web Mercator, UTM, local tangent plane)

```rust
use tpt_gis_core::projection::local_tangent_plane::LocalTangentPlane;
use tpt_gis_core::projection::utm::{self, Zone};
use tpt_gis_core::projection::web_mercator;
use tpt_gis_core::{Ellipsoid, GeoPoint};

let pt = GeoPoint::new(-37.8136, 144.9631);

let xy = web_mercator::forward(pt);
println!("web mercator = ({:.1}, {:.1}) m", xy.x, xy.y);

let zone = Zone::containing(pt);
let utm = utm::forward(&Ellipsoid::WGS84, pt, zone);
println!("UTM {}N easting/northing = ({:.1}, {:.1}) [EPSG {}]",
    zone.number, utm.x, utm.y, zone.epsg_code());

let plane = LocalTangentPlane::new(pt);
let local = plane.to_local(GeoPoint::new(-33.8688, 151.2093));
println!("Sydney is {:.0} m east, {:.0} m north of Melbourne",
    local.x, local.y);
```

### Datum transformation (geodetic ↔ ECEF)

```rust
use tpt_gis_core::datum::{geodetic_to_geocentric, geocentric_to_geodetic, Geocentric};
use tpt_gis_core::{Ellipsoid, GeoPoint};

let geo = GeoPoint::new(-37.8136, 144.9631);
let gc = geodetic_to_geocentric(&Ellipsoid::WGS84, geo, 0.0);
println!("ECEF = ({:.0}, {:.0}, {:.0}) m", gc.x, gc.y, gc.z);

let (back, height) = geocentric_to_geodetic(&Ellipsoid::WGS84, gc);
assert!((back.lat_deg - geo.lat_deg).abs() < 1e-9);
println!("round-trip height = {height}");
```

## `no_std`

The `no_std` path is first-class: every public function uses `libm` for
transcendental math, so the crate cross-compiles to bare-metal targets. The
`drone-geofence-demo` example in the workspace is built `--no-default-features`
for `thumbv7em-none-eabihf` and `riscv32imc-unknown-none-elf`.

## License

Licensed under either of [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0)
or [MIT license](https://opensource.org/licenses/MIT) at your option.
