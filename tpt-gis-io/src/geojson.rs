//! GeoJSON (RFC 7946) reader/writer.
//!
//! Geometry parsing goes through [`serde_json::Value`] rather than a single derived
//! struct: GeoJSON's `coordinates` array is nested to a different depth per geometry
//! type (and `GeometryCollection` uses `geometries` instead of `coordinates` at all),
//! which doesn't map onto one uniform `#[derive(Deserialize)]` shape. `Feature`/
//! `FeatureCollection`, whose shape *is* uniform, use derived (de)serialization.

use core::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::geometry::{Geometry, Point, Polygon};

/// An error encountered while reading or writing GeoJSON.
#[derive(Debug)]
pub enum GeoJsonError {
    /// The input was not valid JSON at all.
    Json(serde_json::Error),
    /// The input was valid JSON but not a well-formed GeoJSON geometry/Feature/
    /// FeatureCollection (e.g. missing `type`, wrong coordinate nesting).
    InvalidGeometry(String),
}

impl fmt::Display for GeoJsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GeoJsonError::Json(e) => write!(f, "invalid JSON: {e}"),
            GeoJsonError::InvalidGeometry(msg) => write!(f, "invalid GeoJSON geometry: {msg}"),
        }
    }
}

impl std::error::Error for GeoJsonError {}

impl From<serde_json::Error> for GeoJsonError {
    fn from(e: serde_json::Error) -> Self {
        GeoJsonError::Json(e)
    }
}

fn coord_from_value(value: &Value) -> Result<Point, GeoJsonError> {
    let arr = value
        .as_array()
        .ok_or_else(|| GeoJsonError::InvalidGeometry("coordinate is not an array".into()))?;
    let x = arr
        .first()
        .and_then(Value::as_f64)
        .ok_or_else(|| GeoJsonError::InvalidGeometry("coordinate missing x".into()))?;
    let y = arr
        .get(1)
        .and_then(Value::as_f64)
        .ok_or_else(|| GeoJsonError::InvalidGeometry("coordinate missing y".into()))?;
    Ok(Point::new(x, y))
}

fn coord_to_value(point: Point) -> Value {
    Value::from(vec![point.x, point.y])
}

fn line_from_value(value: &Value) -> Result<Vec<Point>, GeoJsonError> {
    value
        .as_array()
        .ok_or_else(|| GeoJsonError::InvalidGeometry("expected an array of coordinates".into()))?
        .iter()
        .map(coord_from_value)
        .collect()
}

fn line_to_value(points: &[Point]) -> Value {
    Value::from(points.iter().map(|&p| coord_to_value(p)).collect::<Vec<_>>())
}

fn polygon_from_value(value: &Value) -> Result<Polygon, GeoJsonError> {
    let rings = value
        .as_array()
        .ok_or_else(|| GeoJsonError::InvalidGeometry("expected an array of rings".into()))?
        .iter()
        .map(line_from_value)
        .collect::<Result<Vec<_>, _>>()?;
    if rings.is_empty() {
        return Err(GeoJsonError::InvalidGeometry("polygon has no rings".into()));
    }
    Ok(Polygon { rings })
}

fn polygon_to_value(polygon: &Polygon) -> Value {
    Value::from(polygon.rings.iter().map(|r| line_to_value(r)).collect::<Vec<_>>())
}

/// Parses a GeoJSON geometry object (`{"type": ..., "coordinates": ...}`, or
/// `{"type": "GeometryCollection", "geometries": [...]}`) from its JSON value.
pub fn geometry_from_value(value: &Value) -> Result<Geometry, GeoJsonError> {
    let obj = value
        .as_object()
        .ok_or_else(|| GeoJsonError::InvalidGeometry("geometry is not a JSON object".into()))?;
    let type_ = obj
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| GeoJsonError::InvalidGeometry("geometry missing \"type\"".into()))?;

    if type_ == "GeometryCollection" {
        let geometries = obj
            .get("geometries")
            .and_then(Value::as_array)
            .ok_or_else(|| GeoJsonError::InvalidGeometry("missing \"geometries\"".into()))?
            .iter()
            .map(geometry_from_value)
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(Geometry::GeometryCollection(geometries));
    }

    let coordinates = obj
        .get("coordinates")
        .ok_or_else(|| GeoJsonError::InvalidGeometry("missing \"coordinates\"".into()))?;

    match type_ {
        "Point" => Ok(Geometry::Point(coord_from_value(coordinates)?)),
        "LineString" => Ok(Geometry::LineString(line_from_value(coordinates)?)),
        "Polygon" => Ok(Geometry::Polygon(polygon_from_value(coordinates)?)),
        "MultiPoint" => Ok(Geometry::MultiPoint(line_from_value(coordinates)?)),
        "MultiLineString" => {
            let lines = coordinates
                .as_array()
                .ok_or_else(|| GeoJsonError::InvalidGeometry("expected an array of lines".into()))?
                .iter()
                .map(line_from_value)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Geometry::MultiLineString(lines))
        }
        "MultiPolygon" => {
            let polygons = coordinates
                .as_array()
                .ok_or_else(|| {
                    GeoJsonError::InvalidGeometry("expected an array of polygons".into())
                })?
                .iter()
                .map(polygon_from_value)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Geometry::MultiPolygon(polygons))
        }
        other => Err(GeoJsonError::InvalidGeometry(format!("unknown geometry type \"{other}\""))),
    }
}

/// Serializes a [`Geometry`] to its GeoJSON JSON value representation.
#[must_use]
pub fn geometry_to_value(geometry: &Geometry) -> Value {
    match geometry {
        Geometry::Point(p) => {
            serde_json::json!({"type": "Point", "coordinates": coord_to_value(*p)})
        }
        Geometry::LineString(points) => {
            serde_json::json!({"type": "LineString", "coordinates": line_to_value(points)})
        }
        Geometry::Polygon(polygon) => {
            serde_json::json!({"type": "Polygon", "coordinates": polygon_to_value(polygon)})
        }
        Geometry::MultiPoint(points) => {
            serde_json::json!({"type": "MultiPoint", "coordinates": line_to_value(points)})
        }
        Geometry::MultiLineString(lines) => {
            let coords: Vec<Value> = lines.iter().map(|l| line_to_value(l)).collect();
            serde_json::json!({"type": "MultiLineString", "coordinates": coords})
        }
        Geometry::MultiPolygon(polygons) => {
            let coords: Vec<Value> = polygons.iter().map(polygon_to_value).collect();
            serde_json::json!({"type": "MultiPolygon", "coordinates": coords})
        }
        Geometry::GeometryCollection(geometries) => {
            let geoms: Vec<Value> = geometries.iter().map(geometry_to_value).collect();
            serde_json::json!({"type": "GeometryCollection", "geometries": geoms})
        }
    }
}

/// Parses a GeoJSON geometry object from a JSON string.
pub fn parse_geometry(json: &str) -> Result<Geometry, GeoJsonError> {
    geometry_from_value(&serde_json::from_str(json)?)
}

/// Serializes a [`Geometry`] to a GeoJSON string.
#[must_use]
pub fn write_geometry(geometry: &Geometry) -> String {
    geometry_to_value(geometry).to_string()
}

#[derive(Serialize, Deserialize)]
struct RawFeature {
    #[serde(rename = "type")]
    type_: String,
    geometry: Option<Value>,
    #[serde(default)]
    properties: Map<String, Value>,
}

/// A GeoJSON `Feature`: a geometry plus a JSON object of arbitrary properties.
#[derive(Debug, Clone, PartialEq)]
pub struct Feature {
    /// The feature's geometry (GeoJSON allows this to be `null`).
    pub geometry: Option<Geometry>,
    /// Arbitrary key/value properties attached to the feature.
    pub properties: Map<String, Value>,
}

/// A GeoJSON `FeatureCollection`: an ordered list of [`Feature`]s.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FeatureCollection {
    /// The collection's features, in order.
    pub features: Vec<Feature>,
}

/// Parses a GeoJSON `FeatureCollection` from a JSON string.
pub fn parse_feature_collection(json: &str) -> Result<FeatureCollection, GeoJsonError> {
    let value: Value = serde_json::from_str(json)?;
    let obj = value
        .as_object()
        .ok_or_else(|| GeoJsonError::InvalidGeometry("expected a JSON object".into()))?;
    let type_ = obj.get("type").and_then(Value::as_str);
    if type_ != Some("FeatureCollection") {
        return Err(GeoJsonError::InvalidGeometry(
            "expected \"type\": \"FeatureCollection\"".into(),
        ));
    }
    let raw_features: Vec<RawFeature> = serde_json::from_value(
        obj.get("features")
            .cloned()
            .ok_or_else(|| GeoJsonError::InvalidGeometry("missing \"features\"".into()))?,
    )?;

    let features = raw_features
        .into_iter()
        .map(|raw| {
            Ok(Feature {
                geometry: raw.geometry.map(|g| geometry_from_value(&g)).transpose()?,
                properties: raw.properties,
            })
        })
        .collect::<Result<Vec<_>, GeoJsonError>>()?;

    Ok(FeatureCollection { features })
}

/// Serializes a [`FeatureCollection`] to a GeoJSON string.
#[must_use]
pub fn write_feature_collection(collection: &FeatureCollection) -> String {
    let features: Vec<Value> = collection
        .features
        .iter()
        .map(|f| {
            serde_json::json!({
                "type": "Feature",
                "geometry": f.geometry.as_ref().map(geometry_to_value),
                "properties": f.properties,
            })
        })
        .collect();
    serde_json::json!({"type": "FeatureCollection", "features": features}).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_round_trips() {
        let geometry = Geometry::Point(Point::new(30.0, 10.0));
        let json = write_geometry(&geometry);
        assert_eq!(parse_geometry(&json).unwrap(), geometry);
    }

    #[test]
    fn parses_rfc7946_point_example() {
        // Example directly from RFC 7946 section 3.1.2.
        let geometry = parse_geometry(r#"{"type": "Point", "coordinates": [30.0, 10.0]}"#).unwrap();
        assert_eq!(geometry, Geometry::Point(Point::new(30.0, 10.0)));
    }

    #[test]
    fn parses_rfc7946_polygon_with_hole_example() {
        // Example directly from RFC 7946 section 3.1.6.
        let json = r#"{
            "type": "Polygon",
            "coordinates": [
                [[35.0, 10.0], [45.0, 45.0], [15.0, 40.0], [10.0, 20.0], [35.0, 10.0]],
                [[20.0, 30.0], [35.0, 35.0], [30.0, 20.0], [20.0, 30.0]]
            ]
        }"#;
        let geometry = parse_geometry(json).unwrap();
        match geometry {
            Geometry::Polygon(polygon) => {
                assert_eq!(polygon.rings.len(), 2);
                assert_eq!(polygon.exterior().len(), 5);
                assert_eq!(polygon.interiors()[0].len(), 4);
            }
            other => panic!("expected Polygon, got {other:?}"),
        }
    }

    #[test]
    fn multi_polygon_round_trips() {
        let geometry = Geometry::MultiPolygon(vec![
            Polygon::from_exterior(vec![
                Point::new(0.0, 0.0),
                Point::new(1.0, 0.0),
                Point::new(0.0, 0.0),
            ]),
            Polygon::from_exterior(vec![
                Point::new(5.0, 5.0),
                Point::new(6.0, 5.0),
                Point::new(5.0, 5.0),
            ]),
        ]);
        let json = write_geometry(&geometry);
        assert_eq!(parse_geometry(&json).unwrap(), geometry);
    }

    #[test]
    fn geometry_collection_round_trips() {
        let geometry = Geometry::GeometryCollection(vec![
            Geometry::Point(Point::new(1.0, 2.0)),
            Geometry::LineString(vec![Point::new(0.0, 0.0), Point::new(1.0, 1.0)]),
        ]);
        let json = write_geometry(&geometry);
        assert_eq!(parse_geometry(&json).unwrap(), geometry);
    }

    #[test]
    fn feature_collection_round_trips_with_properties() {
        let json = r#"{
            "type": "FeatureCollection",
            "features": [
                {
                    "type": "Feature",
                    "geometry": {"type": "Point", "coordinates": [1.0, 2.0]},
                    "properties": {"name": "test", "count": 3}
                }
            ]
        }"#;
        let collection = parse_feature_collection(json).unwrap();
        assert_eq!(collection.features.len(), 1);
        assert_eq!(collection.features[0].geometry, Some(Geometry::Point(Point::new(1.0, 2.0))));
        assert_eq!(collection.features[0].properties["name"], "test");

        let round_tripped =
            parse_feature_collection(&write_feature_collection(&collection)).unwrap();
        assert_eq!(round_tripped, collection);
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(parse_geometry("not json").is_err());
    }

    #[test]
    fn rejects_unknown_geometry_type() {
        let result = parse_geometry(r#"{"type": "Sphere", "coordinates": [0.0, 0.0]}"#);
        assert!(matches!(result, Err(GeoJsonError::InvalidGeometry(_))));
    }
}
