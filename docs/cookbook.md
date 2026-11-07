# Cookbook

Short recipes for common tasks. Each one uses only what the crates provide today; see
[Coming from GDAL, PROJ, or GEOS](coming-from-gdal-proj-geos.md) for what is missing.

## Reproject a shapefile to GeoJSON

Convert the shapefile, then reproject it. `convert` reads geometry only; `.dbf`
attributes are not read yet.

```sh
tptgis convert --input districts.shp --output districts.geojson
tptgis reproject --input districts.geojson --crs web-mercator --output districts-3857.geojson
```

## Spatial join: points in polygons

```sh
tptgis spatial-join --points stations.geojson --polygons zones.geojson \
  --format csv --output stations-in-zones.csv
```

Holes in polygons are respected, and multi-part features are joined part by part.

## Nearest polygon to each point

```rust
use tpt_gis_index::spatial_join::{nearest_polygon_join, IndexedPoint, IndexedPolygon};
use tpt_gis_geom::Point;

fn nearest_zone(points: &[IndexedPoint<usize>], zones: &[IndexedPolygon<usize>]) {
    for m in nearest_polygon_join(points, zones) {
        // m.point and m.polygon are indices; m.distance is 0 when the point is inside.
        println!("point {} -> zone {} at {:.3}", m.point, m.polygon, m.distance);
    }
}
```

The search visits polygons in bounding-box order and stops as soon as no remaining
box can beat the best exact distance found.

## Union, difference, and buffer

Enable the `alloc` feature of `tpt-gis-geom` for these. The results are owned rings.

```rust
use tpt_gis_geom::{buffer, clip, Point, Polygon};

let a_ring = [
    Point::new(0.0, 0.0), Point::new(10.0, 0.0), Point::new(10.0, 10.0),
    Point::new(0.0, 10.0), Point::new(0.0, 0.0),
];
let b_ring = [
    Point::new(5.0, 5.0), Point::new(15.0, 5.0), Point::new(15.0, 15.0),
    Point::new(5.0, 15.0), Point::new(5.0, 5.0),
];
let (a, b) = (Polygon::from_exterior(&a_ring), Polygon::from_exterior(&b_ring));

let union_area: f64 = clip::union(&a, &b).iter().map(|p| p.area()).sum();
assert!((union_area - 175.0).abs() < 1e-9);

// A 50 m buffer around a road centreline, with round joins.
let road = [Point::new(0.0, 0.0), Point::new(100.0, 0.0), Point::new(100.0, 80.0)];
let corridor = buffer::buffer_line(&road, 50.0, 32);
```

## Remove and query an R-tree

```rust
use tpt_gis_index::RTree;
use tpt_gis_geom::{Point, Rect};

let mut tree = RTree::new();
let bbox = Rect::new(Point::new(0.0, 0.0), Point::new(1.0, 1.0));
tree.insert(bbox, "depot");

let nearby = tree.nearest(Point::new(3.0, 3.0), 1); // [(distance, &"depot")]
assert!(tree.remove(&bbox, &"depot"));
```

## Geodesic distance and bearing

```rust
use tpt_gis_core::{geodesic, Ellipsoid, GeoPoint};

let melbourne = GeoPoint::new(-37.8136, 144.9631);
let sydney = GeoPoint::new(-33.8688, 151.2093);

let inverse = geodesic::inverse(&Ellipsoid::WGS84, melbourne, sydney).unwrap();
println!("{:.0} m at {:.1}°", inverse.distance_m, inverse.initial_bearing_deg);

// Where do you end up 100 km away on a bearing of 45°?
let dest = geodesic::direct(&Ellipsoid::WGS84, melbourne, 45.0, 100_000.0).unwrap();
println!("{:.4}, {:.4}", dest.destination.lat_deg, dest.destination.lon_deg);
```

Both functions return `GeodesicError::InvalidInput` for non-finite or out-of-range
input, and `ConvergenceFailure` for nearly antipodal points (Vincenty's method).

## Convex hull and polygon checks

```rust
use tpt_gis_geom::{convex_hull, polygon::Polygon, Point};

let mut points = [
    Point::new(0.0, 0.0), Point::new(4.0, 0.0), Point::new(4.0, 4.0),
    Point::new(0.0, 4.0), Point::new(2.0, 2.0), // interior, dropped from the hull
];
let mut hull = [Point::new(0.0, 0.0); 10];
let n = convex_hull(&mut points, &mut hull).expect("output buffer is large enough");
assert_eq!(n, 4);

let ring = [
    Point::new(0.0, 0.0), Point::new(4.0, 4.0), Point::new(4.0, 0.0),
    Point::new(0.0, 4.0), Point::new(0.0, 0.0), // a bow-tie
];
assert!(!Polygon::from_exterior(&ring).is_valid());
```

The hull and simplification functions need caller-supplied buffers, so they work
without an allocator.

## Simplify a line (Douglas-Peucker)

```rust
use tpt_gis_geom::{simplify::simplify_into, Point};

let line = [
    Point::new(0.0, 0.0), Point::new(1.0, 0.01), Point::new(2.0, -0.01), Point::new(3.0, 0.0),
];
let mut keep = [false; 4];
let mut stack = [(0usize, 0usize); 4];
let mut out = [Point::new(0.0, 0.0); 4];
let n = simplify_into(&line, 0.1, &mut keep, &mut stack, &mut out).unwrap();
assert_eq!(&out[..n], &[Point::new(0.0, 0.0), Point::new(3.0, 0.0)]);
```

## Embedded geofence

Compile the no-fly zones once on the host, then include the generated module in
firmware:

```sh
tptgis geofence-compile --input no-fly.geojson --output firmware/src/fence.rs
```

The generated module needs only `tpt-gis-geom`. See the
[CLI guide](cli-usage.md#geofence-compile) for a complete check function.

## Lambert-93 coordinates for a French address

```rust
use tpt_gis_core::projection::lcc::LambertConformal;
use tpt_gis_core::GeoPoint;

let lambert93 = LambertConformal::lambert93();
let xy = lambert93.forward(GeoPoint::new(48.8566, 2.3522));
let back = lambert93.inverse(xy);
```

Lambert-93 is the EPSG:2154 parameterisation of the conic in `tpt_gis_core::projection::lcc`.
It is not registered in the EPSG table, so `epsg::lookup(2154)` returns `None`.

## Read a window of a remote COG

Requires the `http` feature of `tpt-gis-raster`. Only the tiles covering the window are
downloaded.

```rust
use tpt_gis_raster::http::CogRangeReader;

async fn top_left(url: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Headers can carry credentials; only http and https URLs are accepted.
    let reader = CogRangeReader::with_headers(url, [("Authorization", "Bearer TOKEN")]);
    let window = reader.read_region(0, 0, 1024, 1024).await?;
    println!("{} x {}", window.width(), window.height());
    Ok(())
}
```

Redirects are followed up to five times, and only to http or https URLs.
