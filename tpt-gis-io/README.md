# tpt-gis-io

Parsers and writers for GeoJSON, WKB/WKT, Shapefile, and GeoPackage for the
[`tpt-gis`](https://github.com/tpt-solutions/tpt-gis) engine.

Unlike the `no_std` engine crates, `tpt-gis-io` is a host-native crate: parsing
variable-sized file and stream data inherently needs heap allocation, so there is no
reason to fight that here.

> **Status:** not yet published to crates.io. Depend on it by path or git, for example
> `tpt-gis-io = { git = "https://github.com/tpt-solutions/tpt-gis" }`.

## Why

A uniform `Geometry` type in the middle and battle-tested readers and writers around it.
Every format produces and consumes the same owned `Geometry` / `Feature` types, so you
can read a Shapefile and write GeoJSON without an intermediate conversion step — and
without any C FFI (the GeoPackage reader and writer are pure Rust, no `libsqlite3-sys`).

## Installation

```toml
[dependencies]
tpt-gis-io = { git = "https://github.com/tpt-solutions/tpt-gis" }
```

## Modules

- **`geometry`** — the shared owned `Geometry` / `Polygon` types produced and consumed by every
  reader and writer. `Polygon::as_geom` bridges to `tpt-gis-geom` predicates.
- **`geojson`** — `parse_feature_collection` / `write_feature_collection` (RFC 7946), plus
  `parse_geometry` / `write_geometry`.
- **`wkb`** — `parse_geometry` / `write_geometry` for Well-Known Binary (big- and little-endian),
  including ISO Z/M and EWKB forms.
- **`wkt`** — `parse_geometry` / `write_geometry` for Well-Known Text, including dimension tags
  (`Z`, `M`, `ZM`) and `SRID=`.
- **`shapefile`** — `read_shp`, `read_shp_records`, `read_dbf` / `read_dbf_with`, and a streaming
  `ShpGeometryIter`. Attributes are decoded with the `.cpg` encoding.
- **`shapefile_writer`** — `write_shapefile` (`.shp` and `.shx`) and `write_dbf`.
- **`geopackage`** — `read_gpkg` / `write_gpkg` for `.gpkg` (pure-Rust SQLite file format and
  GeoPackageBinary codec, zero FFI). The writer can add the R-tree spatial index.

## Examples

### GeoJSON round-trip

```rust
use tpt_gis_io::geojson::parse_feature_collection;

let json = r#"{
  "type": "FeatureCollection",
  "features": [
    {"type": "Feature",
     "geometry": {"type": "Point", "coordinates": [1.0, 2.0]},
     "properties": {"name": "alpha"}}
  ]
}"#;

let fc = parse_feature_collection(json).unwrap();
assert_eq!(fc.features.len(), 1);
println!("feature name = {}", fc.features[0].properties["name"]);
```

### WKT and WKB

```rust
use tpt_gis_io::geometry::{Geometry, Point};
use tpt_gis_io::{wkb, wkt};

let pt = Geometry::Point(Point::new(3.0, 4.0));

let wkt_str = wkt::write_geometry(&pt);
let parsed = wkt::parse_geometry("POINT (3 4)").unwrap();
assert_eq!(parsed, pt);
println!("WKT: {wkt_str}");

let bytes = wkb::write_geometry(&pt);
let from_wkb = wkb::parse_geometry(&bytes).unwrap();
assert_eq!(from_wkb, pt);
```

### GeoPackage: write then read

```rust
use serde_json::json;
use tpt_gis_io::geojson::Feature;
use tpt_gis_io::geometry::{Geometry, Point};
use tpt_gis_io::geopackage::{read_gpkg, write_gpkg, WriteOptions};

let feature = Feature {
    geometry: Some(Geometry::Point(Point::new(1.0, 2.0))),
    properties: [("name".into(), json!("alpha"))].into_iter().collect(),
};

let bytes = write_gpkg(
    &[feature],
    &WriteOptions {
        table_name: "features".into(),
        geometry_column: "geom".into(),
        srs_id: 4326,
        id_column: None,
        description: None,
        spatial_index: false, // true writes the gpkg_rtree_index extension
    },
).unwrap();

let read_back = read_gpkg(&bytes).unwrap();
assert_eq!(read_back.len(), 1);
println!("read back geometry = {:?}", read_back[0].geometry);
```

### Bridging to `tpt-gis-geom`

```rust
use tpt_gis_geom::predicates;
use tpt_gis_io::geometry::{Point, Polygon};

let polygon = Polygon::from_exterior(vec![
    Point::new(0.0, 0.0),
    Point::new(10.0, 0.0),
    Point::new(10.0, 10.0),
    Point::new(0.0, 10.0),
    Point::new(0.0, 0.0),
]);

let mut scratch = Vec::new();
let geom = polygon.as_geom(&mut scratch);
assert!(predicates::point_in_ring(
    tpt_gis_geom::Point::new(5.0, 5.0),
    geom.exterior,
));
```

## Scope notes

- **Shapefile** reading and writing cover `.shp`, `.shx` and `.dbf`. A `.prj` sidecar is written
  by the `tptgis` CLI, not by this crate. Z and M shape types are read as 2D. Mixed shape types
  and geometry collections are rejected on write.
- **GeoPackage writer** writes rowid tables with interior pages and overflow records. It can
  write the R-tree spatial index, but not the tile or attribute extensions. The reader also
  reads `WITHOUT ROWID` tables.

## License

Licensed under either of [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0)
or [MIT license](https://opensource.org/licenses/MIT) at your option.
