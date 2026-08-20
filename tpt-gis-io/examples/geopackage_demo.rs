//! `tpt-gis-io` example: writing and reading a GeoPackage (`.gpkg`) entirely in
//! memory — no external SQLite engine — plus a GeoJSON round-trip of the result.
//!
//! Run with: `cargo run --example geopackage_demo -p tpt-gis-io`

use tpt_gis_io::geojson::{write_feature_collection, Feature, FeatureCollection};
use tpt_gis_io::geometry::{Geometry, Point};
use tpt_gis_io::geopackage::{read_gpkg, write_gpkg, GeoPackageError, WriteOptions};

fn main() -> Result<(), GeoPackageError> {
    // Build a couple of point features with attributes.
    let mut props_a = serde_json::Map::new();
    props_a.insert("name".into(), serde_json::json!("alpha"));
    props_a.insert("count".into(), serde_json::json!(3));
    let f1 = Feature { geometry: Some(Geometry::Point(Point::new(1.0, 2.0))), properties: props_a };

    let mut props_b = serde_json::Map::new();
    props_b.insert("name".into(), serde_json::json!("beta"));
    props_b.insert("count".into(), serde_json::json!(7));
    let f2 = Feature { geometry: Some(Geometry::Point(Point::new(3.0, 4.0))), properties: props_b };

    let features = vec![f1, f2];

    // Write a minimal, valid .gpkg byte buffer (single-leaf-page tables only).
    let opts = WriteOptions {
        table_name: "cities".into(),
        geometry_column: "geom".into(),
        srs_id: 4326,
        id_column: Some("fid".into()),
        description: Some("demo layer".into()),
    };
    let bytes = write_gpkg(&features, &opts)?;
    println!("wrote {} gpkg bytes", bytes.len());

    // Read it back and confirm round-trip fidelity.
    let read = read_gpkg(&bytes)?;
    println!("read {} features", read.len());
    for f in &read {
        let name = f.properties.get("name").and_then(|v| v.as_str()).unwrap_or("");
        println!("  feature '{name}': {:?}", f.geometry);
    }
    assert_eq!(read.len(), 2);

    // The same features as a GeoJSON FeatureCollection.
    let fc = FeatureCollection { features: read };
    let json = write_feature_collection(&fc);
    println!("GeoJSON: {json}");
    Ok(())
}
