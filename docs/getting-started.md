# Getting Started with tpt-gis

tpt-gis is a pure-Rust GIS engine. The crates are not on crates.io yet, so for now
depend on them by path or git.

## Installation

Use the facade crate for everything in one dependency, or pick individual crates:

```toml
[dependencies]
# Everything, via the facade (features: core, geom, io, index, raster, http, std):
tpt-gis = { path = "../tpt-gis" }

# Or individual crates:
tpt-gis-core = { path = "../tpt-gis/tpt-gis-core" }
tpt-gis-geom = { path = "../tpt-gis/tpt-gis-geom" }
tpt-gis-io = { path = "../tpt-gis/tpt-gis-io" }
tpt-gis-index = { path = "../tpt-gis/tpt-gis-index" }
tpt-gis-raster = { path = "../tpt-gis/tpt-gis-raster" }
```

The facade's `prelude` brings the common types into scope:

```rust
use tpt_gis::prelude::*;
```

`tpt-gis-core` and `tpt-gis-geom` are `no_std` by default, so they build for embedded
targets. See the README for the cross-compile commands.

## Geometry

Geometries borrow their coordinates, so they can be built over `static` data without
allocating:

```rust
use tpt_gis_geom::{polygon::Polygon, Point};

const SQUARE: [Point; 5] = [
    Point::new(0.0, 0.0),
    Point::new(10.0, 0.0),
    Point::new(10.0, 10.0),
    Point::new(0.0, 10.0),
    Point::new(0.0, 0.0),
];

let polygon = Polygon::from_exterior(&SQUARE);
assert!(polygon.contains_point(Point::new(5.0, 5.0)));
assert_eq!(polygon.area(), 100.0);
assert!(polygon.is_valid());
```

With the `alloc` feature, `tpt_gis_geom::clip` provides `union`, `intersection`, `difference`
and `symmetric_difference`, and `tpt_gis_geom::buffer` provides round-join buffers for points,
lines, and polygons (holes shrink; negative distances are not supported).

## Reading and writing vector data

`tpt-gis-io` reads and writes GeoJSON, WKT, WKB, and GeoPackage, and reads Shapefile:

```rust
use tpt_gis_io::geojson::parse_geometry;

let geometry = parse_geometry(r#"{"type": "Point", "coordinates": [30.0, 10.0]}"#).unwrap();
```

## Spatial join

```rust
use tpt_gis_index::spatial_join::{spatial_join, IndexedPoint, IndexedPolygon, SpatialJoinConfig};
use tpt_gis_geom::Point;
use tpt_gis_io::geometry::Polygon;

let points = vec![IndexedPoint { point: Point::new(5.0, 5.0), payload: 0 }];
let polygons = vec![IndexedPolygon {
    polygon: Polygon::from_exterior(vec![
        Point::new(0.0, 0.0),
        Point::new(10.0, 0.0),
        Point::new(10.0, 10.0),
        Point::new(0.0, 10.0),
        Point::new(0.0, 0.0),
    ]),
    payload: 0,
}];

let results = spatial_join(&points, &polygons, SpatialJoinConfig::default());
```

`tpt-gis-index` also has a quadtree, H3 and S2 cell ids, nearest-neighbour search
(`RTree::nearest`), removal (`RTree::remove`), and polygon-polygon joins.

## Geodesy and projections

```rust
use tpt_gis_core::{geodesic, Ellipsoid, GeoPoint};
use tpt_gis_core::projection::lcc::LambertConformal;

let flinders_peak = GeoPoint::new(-37.9510, 144.4249);
let buninyong = GeoPoint::new(-37.6528, 143.9265);
let result = geodesic::inverse(&Ellipsoid::WGS84, flinders_peak, buninyong).unwrap();
println!("Distance: {:.3} m", result.distance_m);

// Lambert-93 (EPSG:2154), the French national grid.
let xy = LambertConformal::lambert93().forward(GeoPoint::new(48.8566, 2.3522));
```

Projections are plain types and functions: `Web Mercator`, `UTM`, `Lambert Conformal Conic`
(`LambertConformal`), and `Albers Equal-Area` (`Albers`). There is no general
`transform(from, to)` or EPSG registry entry for them yet.

## Rasters

`tpt-gis-raster` reads GeoTIFF and validates Cloud-Optimized GeoTIFF layout:

```rust
use tpt_gis_raster::geotiff;
use std::path::Path;

let band = geotiff::read_file(Path::new("elevation.tif")).unwrap();
println!("{} x {}", band.width(), band.height());
```

With the `http` feature, `tpt_gis_raster::http::CogRangeReader` reads a remote COG by
fetching only the tiles a window needs (see the [cookbook](cookbook.md)).

## Command line

The `tptgis` binary covers the common tasks without writing Rust:

```sh
cargo run -p tpt-gis-cli --bin tptgis -- info roads.gpkg
cargo run -p tpt-gis-cli --bin tptgis -- convert --input roads.shp --output roads.geojson
cargo run -p tpt-gis-cli --bin tptgis -- spatial-join --points points.geojson --polygons zones.geojson
```

See [CLI usage](cli-usage.md) for every command.

## Python

`tpt-gis-py` is a pyo3 + maturin extension with geometry, format, and geodesy
functions. See [Python bindings](python-bindings.md).

## Browser

`examples/wasm-geofence-demo` runs the geofence check in the browser through
`wasm-bindgen`.

## Next steps

- [Cookbook](cookbook.md): short recipes for common tasks.
- [Coming from GDAL, PROJ, or GEOS](coming-from-gdal-proj-geos.md): where each
  familiar operation lives, and what is missing.
- [Benchmarks](benchmarks.md).
