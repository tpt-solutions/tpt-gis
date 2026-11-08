# tpt-gis-core

Coordinate reference systems, datum transformations, and geodesic math for the
[`tpt-gis`](https://github.com/tpt-solutions/tpt-gis) engine.

This crate is `no_std` by default (it uses [`libm`] for transcendental math so it
runs on embedded targets); enable the `std` feature to use the platform's native
math intrinsics.

> **Status:** not yet published to crates.io. Depend on it by path or git, for example
> `tpt-gis-core = { git = "https://github.com/tpt-solutions/tpt-gis" }`.

## Why

Before you reach for a GDAL/PROJ binding, much of the day-to-day GIS math — "how far
is A from B on the Earth?", "what's the UTM easting of this coordinate?", "shift
this point from one datum to another" — is a handful of well-understood formulas.
`tpt-gis-core` implements those formulas from scratch in safe, `no_std` Rust.

## Features

| Feature | Default | Description                                      |
| ------- | ------- | ------------------------------------------------ |
| `std`   | off     | Use the platform's native `f64` math intrinsics. |

## Modules

- **`ellipsoid`** — `Ellipsoid` reference models (`WGS84`, `GRS80`) and their
  semi-axis and eccentricity parameters.
- **`geodesic`** — Karney inverse (distance and initial and final bearing) and direct
  (destination from a start point, bearing and distance) on an ellipsoid. Also
  `polygon_measure` (perimeter and area of a polygon on the ellipsoid) and `sample`
  (points at equal steps along a geodesic).
- **`projection`**
  - `web_mercator` — WGS84 ↔ Web Mercator (EPSG:3857), in metres.
  - `utm` — WGS84 ↔ UTM (forward and inverse) with `Zone` selection (EPSG:326xx / 327xx).
  - `transverse_mercator` — generic Transverse Mercator.
  - `lcc` — Lambert Conformal Conic, with `Lambert93` (EPSG:2154) as a preset.
  - `albers` — Albers Equal-Area, with `conus_albers` (EPSG:5070) as a preset.
  - `transverse_mercator` — Transverse Mercator with any origin, with `british_national_grid`
    (EPSG:27700) as a preset.
  - `local_tangent_plane` — local East-North-Up approximation for small areas
    (correctly accounts for longitude shrinking with `cos(latitude)`).
- **`crs`** — the `Crs` enum (`Wgs84`, `WebMercator`, `Utm`, `Lambert93`, `ConusAlbers`, `Osgb36`,
  `BritishNationalGrid`) and
  `transform`, which converts a point between any two of them.
- **`epsg`** — `lookup`, which maps an EPSG code to a `Crs`.
- **`datum`** — geodetic ↔ geocentric (ECEF) conversion and 7-parameter Helmert
  (Bursa-Wolf) datum transformation.

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
let dest = geodesic::direct(&Ellipsoid::WGS84, melbourne, 45.0, 100_000.0).unwrap();
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

### Transforming between supported CRSs

```rust
use tpt_gis_core::crs::{transform, Crs};
use tpt_gis_core::PlanarPoint;

// Paris, as a WGS84 longitude/latitude pair, into Lambert-93 metres.
let wgs84 = PlanarPoint::new(2.3522, 48.8566);
let lambert = transform(wgs84, Crs::Wgs84, Crs::Lambert93);
println!("Lambert-93 = ({:.1}, {:.1})", lambert.x, lambert.y);
```

### Datum transformation (geodetic ↔ ECEF)

```rust
use tpt_gis_core::datum::{geodetic_to_geocentric, geocentric_to_geodetic};
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
transcendental math, so the crate cross-compiles to bare-metal targets. CI builds
this crate for `thumbv7em-none-eabihf`, `riscv32imc-unknown-none-elf` and
`wasm32-unknown-unknown`.

## License

Licensed under either of [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0)
or [MIT license](https://opensource.org/licenses/MIT) at your option.

[`libm`]: https://crates.io/crates/libm
