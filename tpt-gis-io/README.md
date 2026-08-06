# tpt-gis-io

Parsers and writers for GeoJSON, WKB/WKT, Shapefile, and GeoPackage for the
[`tpt-gis`](https://crates.io/crates/tpt-gis) engine.

Unlike the `no_std` engine crates, `tpt-gis-io` is a host-native crate: parsing
variable-sized file and stream data inherently needs heap allocation, so there
is no reason to fight that here.

## Installation

```sh
cargo add tpt-gis-io
```

## Example

```rust
use tpt_gis_io::geojson::parse_feature_collection;

let json = r#"{
  "type": "FeatureCollection",
  "features": [
    {"type": "Feature", "geometry": {"type": "Point", "coordinates": [1.0, 2.0]}, "properties": {}}
  ]
}"#;

let fc = parse_feature_collection(json).unwrap();
assert_eq!(fc.features.len(), 1);
```

The same `Feature` / `Geometry` types are produced and consumed by every format
reader and writer:

- **GeoJSON** — `geojson::parse_feature_collection` / `write_feature_collection`.
- **WKB** — `wkb::parse_geometry` / `write_geometry`.
- **WKT** — `wkt::parse_geometry` / `write_geometry`.
- **Shapefile** — `shapefile::read_shp` / `read_dbf` (reader only).
- **GeoPackage** — `geopackage::read_gpkg` / `write_gpkg` (pure-Rust SQLite file
  format, zero FFI).

## License

Licensed under either of [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0)
or [MIT license](https://opensource.org/licenses/MIT) at your option.
