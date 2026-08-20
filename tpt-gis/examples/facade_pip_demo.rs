//! `tpt-gis` facade example: read GeoJSON points and polygons and run a
//! points-in-polygons spatial join, entirely through the single facade crate.
//!
//! Run with: `cargo run --example facade_pip_demo -p tpt-gis`

use serde_json::Map;
use tpt_gis::index::spatial_join::{spatial_join, IndexedPoint, IndexedPolygon, SpatialJoinConfig};
use tpt_gis::io::geojson::parse_feature_collection;
use tpt_gis::io::geometry::Geometry;

fn main() {
    let points_json = r#"{
        "type": "FeatureCollection",
        "features": [
            {"type":"Feature","geometry":{"type":"Point","coordinates":[5,5]},"properties":{"id":1}},
            {"type":"Feature","geometry":{"type":"Point","coordinates":[50,50]},"properties":{"id":2}}
        ]
    }"#;

    let poly_json = r#"{
        "type": "FeatureCollection",
        "features": [
            {"type":"Feature",
             "geometry":{"type":"Polygon","coordinates":[[[0,0],[10,0],[10,10],[0,10],[0,0]]]},
             "properties":{"zone":"A"}}
        ]
    }"#;

    let points_fc = parse_feature_collection(points_json).unwrap();
    let poly_fc = parse_feature_collection(poly_json).unwrap();

    let points: Vec<IndexedPoint<Map<String, serde_json::Value>>> = points_fc
        .features
        .into_iter()
        .filter_map(|f| match f.geometry {
            Some(Geometry::Point(p)) => Some(IndexedPoint { point: p, payload: f.properties }),
            _ => None,
        })
        .collect();

    let polygons: Vec<IndexedPolygon<Map<String, serde_json::Value>>> = poly_fc
        .features
        .into_iter()
        .filter_map(|f| match f.geometry {
            Some(Geometry::Polygon(p)) => {
                Some(IndexedPolygon { polygon: p, payload: f.properties })
            }
            _ => None,
        })
        .collect();

    let results = spatial_join(&points, &polygons, SpatialJoinConfig::default());
    println!("{} point(s) fall inside the polygon:", results.len());
    for r in &results {
        let id = r.point_payload.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
        let zone = r.polygon_payload.get("zone").and_then(|v| v.as_str()).unwrap_or("");
        println!("  point {id} is in zone '{zone}' at ({}, {})", r.point.x, r.point.y);
    }
    assert_eq!(results.len(), 1);
}
