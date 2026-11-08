//! Browser/WASM playground: load GeoJSON, reproject and join entirely client-side.
//!
//! Everything here runs in the browser; no data leaves the page. The two operations are
//! exposed to JavaScript:
//!
//! * [`reproject`] moves every coordinate of a GeoJSON `FeatureCollection` from one EPSG
//!   code to another, using `tpt_gis_core::crs::transform`.
//! * [`join`] performs a points-in-polygons join. Each matching point is returned with the
//!   polygon's properties merged in; point properties win on a name clash.
//!
//! Supported CRSs are those `tpt_gis_core::epsg::lookup` knows (4326, 3857, UTM zones,
//! 2154, 5070, 4277 and 27700). GeoJSON is lon/lat (EPSG:4326) by convention; the page
//! reprojects if a layer is in another CRS.
//!
//! Build and run instructions are in this crate's `README.md`.

use std::fmt::Display;

use serde_json::{Map, Value};
use tpt_gis_core::crs::{transform, Crs};
use tpt_gis_core::{epsg, PlanarPoint};
use tpt_gis_geom::Point;
use tpt_gis_index::spatial_join::{spatial_join, IndexedPoint, IndexedPolygon, SpatialJoinConfig};
use tpt_gis_io::geojson::{parse_feature_collection, write_feature_collection, FeatureCollection};
use tpt_gis_io::geometry::{Geometry, Polygon};
use wasm_bindgen::prelude::*;

/// Converts any displayable error into a JavaScript `Error`.
fn js_error(e: impl Display) -> JsError {
    JsError::new(&e.to_string())
}

/// Looks up an EPSG code, reporting unsupported codes by name.
fn crs_for(code: u32) -> Result<Crs, JsError> {
    epsg::lookup(code).ok_or_else(|| JsError::new(&format!("unsupported EPSG:{code}")))
}

/// Reprojects every coordinate of a GeoJSON `FeatureCollection` from `from_epsg` to
/// `to_epsg` and returns the result as GeoJSON text.
///
/// Feature properties are kept. Coordinates are transformed as `(x, y)` in the source CRS;
/// for EPSG:4326 that is `(lon, lat)`.
///
/// # Errors
///
/// Returns an error if either EPSG code is unsupported or the input is not a GeoJSON
/// `FeatureCollection`.
#[wasm_bindgen]
pub fn reproject(geojson: &str, from_epsg: u32, to_epsg: u32) -> Result<String, JsError> {
    let from = crs_for(from_epsg)?;
    let to = crs_for(to_epsg)?;
    let mut collection = parse_feature_collection(geojson).map_err(js_error)?;
    let convert = |p: Point| {
        let q = transform(PlanarPoint::new(p.x, p.y), from, to);
        Point { x: q.x, y: q.y }
    };
    for feature in &mut collection.features {
        feature.geometry = feature.geometry.take().map(|g| map_points(g, &convert));
    }
    Ok(write_feature_collection(&collection))
}

/// Applies `f` to every vertex of `geometry`, keeping its structure.
fn map_points(geometry: Geometry, f: &impl Fn(Point) -> Point) -> Geometry {
    let ring = |r: Vec<Point>| r.into_iter().map(f).collect::<Vec<_>>();
    match geometry {
        Geometry::Point(p) => Geometry::Point(f(p)),
        Geometry::LineString(l) => Geometry::LineString(ring(l)),
        Geometry::Polygon(p) => {
            Geometry::Polygon(Polygon { rings: p.rings.into_iter().map(ring).collect() })
        }
        Geometry::MultiPoint(ps) => Geometry::MultiPoint(ring(ps)),
        Geometry::MultiLineString(ls) => {
            Geometry::MultiLineString(ls.into_iter().map(ring).collect())
        }
        Geometry::MultiPolygon(ps) => Geometry::MultiPolygon(
            ps.into_iter()
                .map(|p| Polygon { rings: p.rings.into_iter().map(ring).collect() })
                .collect(),
        ),
        Geometry::GeometryCollection(gs) => {
            Geometry::GeometryCollection(gs.into_iter().map(|g| map_points(g, f)).collect())
        }
    }
}

/// Joins point features to the polygons that contain them.
///
/// `points_geojson` and `polygons_geojson` must already be in the same CRS (use
/// [`reproject`] first if they are not). Only `Point` features take part on the point side,
/// and only `Polygon` / `MultiPolygon` features on the polygon side; other features are
/// ignored. A point inside several polygons is returned once per polygon.
///
/// The result is a GeoJSON `FeatureCollection` of the matched points. Each keeps its own
/// properties, plus any property of the containing polygon that the point does not already
/// have.
///
/// # Errors
///
/// Returns an error if either input is not a GeoJSON `FeatureCollection`.
#[wasm_bindgen]
pub fn join(points_geojson: &str, polygons_geojson: &str) -> Result<String, JsError> {
    let points_fc = parse_feature_collection(points_geojson).map_err(js_error)?;
    let polygons_fc = parse_feature_collection(polygons_geojson).map_err(js_error)?;

    let points = indexed_points(points_fc);
    let polygons = indexed_polygons(polygons_fc);
    let matches = spatial_join(&points, &polygons, SpatialJoinConfig::default());

    let features = matches
        .into_iter()
        .map(|m| {
            let mut properties = m.point_payload;
            for (key, value) in m.polygon_payload {
                properties.entry(key).or_insert(value);
            }
            tpt_gis_io::geojson::Feature { geometry: Some(Geometry::Point(m.point)), properties }
        })
        .collect();

    Ok(write_feature_collection(&FeatureCollection { features }))
}

/// Keeps the `Point` features of a collection, with their properties as payloads.
fn indexed_points(collection: FeatureCollection) -> Vec<IndexedPoint<Map<String, Value>>> {
    collection
        .features
        .into_iter()
        .filter_map(|f| match f.geometry {
            Some(Geometry::Point(point)) => Some(IndexedPoint { point, payload: f.properties }),
            _ => None,
        })
        .collect()
}

/// Keeps the polygonal features of a collection. Each part of a `MultiPolygon` becomes its
/// own entry, all carrying the feature's properties.
fn indexed_polygons(collection: FeatureCollection) -> Vec<IndexedPolygon<Map<String, Value>>> {
    let mut out = Vec::new();
    for f in collection.features {
        let parts = match f.geometry {
            Some(Geometry::Polygon(p)) => vec![p],
            Some(Geometry::MultiPolygon(ps)) => ps,
            _ => continue,
        };
        for polygon in parts {
            out.push(IndexedPolygon { polygon, payload: f.properties.clone() });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const ZONES: &str = r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"zone":"A"},"geometry":{"type":"Polygon",
            "coordinates":[[[0,0],[10,0],[10,10],[0,10],[0,0]]]}}]}"#;

    const POINTS: &str = r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"name":"inside"},"geometry":{"type":"Point","coordinates":[5,5]}},
        {"type":"Feature","properties":{"name":"outside"},"geometry":{"type":"Point","coordinates":[20,20]}}]}"#;

    #[test]
    fn join_keeps_only_matching_points_and_merges_polygon_properties() {
        let out = join(POINTS, ZONES).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        let features = v["features"].as_array().unwrap();
        assert_eq!(features.len(), 1);
        assert_eq!(features[0]["properties"]["name"], "inside");
        assert_eq!(features[0]["properties"]["zone"], "A");
    }

    #[test]
    fn reproject_web_mercator_of_origin_is_origin() {
        let input = r#"{"type":"FeatureCollection","features":[
            {"type":"Feature","properties":{},"geometry":{"type":"Point","coordinates":[0,0]}}]}"#;
        let out = reproject(input, 4326, 3857).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        let c = &v["features"][0]["geometry"]["coordinates"];
        assert!(c[0].as_f64().unwrap().abs() < 1e-6);
        assert!(c[1].as_f64().unwrap().abs() < 1e-6);
    }

    #[test]
    fn reproject_london_to_british_national_grid() {
        // Central London (about 0.1278 W, 51.5074 N). The BNG easting/northing should be
        // close to the published values for that location (about 530 000, 180 000).
        let input = r#"{"type":"FeatureCollection","features":[
            {"type":"Feature","properties":{},"geometry":{"type":"Point","coordinates":[-0.1278,51.5074]}}]}"#;
        let out = reproject(input, 4326, 27700).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        let c = &v["features"][0]["geometry"]["coordinates"];
        assert!((c[0].as_f64().unwrap() - 530_000.0).abs() < 2_000.0);
        assert!((c[1].as_f64().unwrap() - 180_000.0).abs() < 2_000.0);
    }
}
