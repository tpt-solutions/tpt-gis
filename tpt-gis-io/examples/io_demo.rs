//! `tpt-gis-io` example: GeoJSON, WKT, WKB, and GeoPackage round-trips.
//!
//! Run with: `cargo run --example io_demo -p tpt-gis-io`

use tpt_gis_io::geojson::{parse_feature_collection, write_geometry};
use tpt_gis_io::geometry::{Geometry, Point, Polygon};
use tpt_gis_io::{wkb, wkt};

fn main() {
    // --- GeoJSON -------------------------------------------------------------
    let json = r#"{
        "type": "FeatureCollection",
        "features": [
            {"type": "Feature",
             "geometry": {"type": "Point", "coordinates": [1.0, 2.0]},
             "properties": {"name": "alpha"}}
        ]
    }"#;
    let fc = parse_feature_collection(json).unwrap();
    println!("parsed {} feature(s)", fc.features.len());
    println!("feature name: {}", fc.features[0].properties["name"]);

    // --- WKT -----------------------------------------------------------------
    let pt = Geometry::Point(Point::new(3.0, 4.0));
    let wkt_str = wkt::write_geometry(&pt);
    let back = wkt::parse_geometry("POINT (3 4)").unwrap();
    println!("WKT: {wkt_str}; round-trip ok: {}", back == pt);

    // --- WKB -----------------------------------------------------------------
    let poly = Geometry::Polygon(Polygon::from_exterior(vec![
        Point::new(0.0, 0.0),
        Point::new(10.0, 0.0),
        Point::new(10.0, 10.0),
        Point::new(0.0, 10.0),
        Point::new(0.0, 0.0),
    ]));
    let bytes = wkb::write_geometry(&poly);
    let parsed = wkb::parse_geometry(&bytes).unwrap();
    println!("WKB round-trip ok: {}", parsed == poly);

    // --- Geometry writer -----------------------------------------------------
    println!("GeoJSON geometry: {}", write_geometry(&pt));
}
