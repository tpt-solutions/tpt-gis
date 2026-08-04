# Getting Started with tpt-gis

## Installation

Add the crates you need to your `Cargo.toml`:

```toml
[dependencies]
tpt-gis-core = "0.1"
tpt-gis-geom = "0.1"
tpt-gis-io = "0.1"
tpt-gis-index = "0.1"
```

## Quick Start

### Geometry Operations

```rust
use tpt_gis_geom::{Point, Polygon, predicates};

let square = [
    Point::new(0.0, 0.0),
    Point::new(10.0, 0.0),
    Point::new(10.0, 10.0),
    Point::new(0.0, 10.0),
    Point::new(0.0, 0.0),
];
let polygon = Polygon::from_exterior(&square);
assert!(polygon.contains_point(Point::new(5.0, 5.0)));
```

### Reading GeoJSON

```rust
use tpt_gis_io::geojson::parse_geometry;

let geometry = parse_geometry(r#"{"type": "Point", "coordinates": [30.0, 10.0]}"#).unwrap();
```

### Spatial Join

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

### Geodesic Calculations

```rust
use tpt_gis_core::{geodesic, Ellipsoid, GeoPoint};

let flinders_peak = GeoPoint::new(-37.9510, 144.4249);
let buninyong = GeoPoint::new(-37.6528, 143.9265);
let result = geodesic::inverse(&Ellipsoid::WGS84, flinders_peak, buninyong).unwrap();
println!("Distance: {:.3} m", result.distance_m);
```

## CLI Usage

The `spatial-join-cli` example provides three subcommands:

```sh
# Run a spatial join
cargo run -p spatial-join-cli -- spatial-join \
  --points points.geojson \
  --polygons polygons.geojson \
  --output results.geojson

# Reproject coordinates
cargo run -p spatial-join-cli -- reproject \
  --input data.geojson \
  --crs web-mercator \
  --output reprojected.geojson

# Generate test fixtures
cargo run -p spatial-join-cli -- generate-fixtures \
  --points 10000 \
  --polygons 500 \
  --output ./fixtures
```
