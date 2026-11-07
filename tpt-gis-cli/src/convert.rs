//! Format conversion and summaries for the `convert` and `info` subcommands.
//!
//! Formats are chosen from file extensions (or an explicit `--from` / `--to`):
//!
//! | format      | extensions          | read | write |
//! |-------------|---------------------|------|-------|
//! | GeoJSON     | `.geojson`, `.json` | yes  | yes   |
//! | WKT         | `.wkt`              | yes  | yes   |
//! | WKB         | `.wkb`              | yes  | yes   |
//! | Shapefile   | `.shp`              | yes  | no    |
//! | GeoPackage  | `.gpkg`             | yes  | yes   |
//!
//! Shapefile attributes (`.dbf`) are not read yet, so shapefile features carry no
//! properties. WKT input holds one geometry per non-empty line. WKB output wraps all
//! features in a single `GeometryCollection`, because a WKB file holds one geometry.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Map, Value};
use tpt_gis_io::geojson::{
    parse_feature_collection, write_feature_collection, Feature, FeatureCollection,
};
use tpt_gis_io::geometry::{Geometry, Point};
use tpt_gis_io::geopackage::{read_gpkg, write_gpkg, WriteOptions};

/// A supported vector file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// GeoJSON `FeatureCollection`.
    GeoJson,
    /// Well-Known Text, one geometry per line.
    Wkt,
    /// Well-Known Binary, one geometry per file.
    Wkb,
    /// ESRI Shapefile (`.shp`; geometry only).
    Shapefile,
    /// OGC GeoPackage (SQLite container).
    GeoPackage,
}

impl Format {
    /// Parses a format name such as `geojson`, `wkt`, `wkb`, `shp`, or `gpkg`.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "geojson" | "json" => Some(Self::GeoJson),
            "wkt" => Some(Self::Wkt),
            "wkb" => Some(Self::Wkb),
            "shp" | "shapefile" => Some(Self::Shapefile),
            "gpkg" | "geopackage" => Some(Self::GeoPackage),
            _ => None,
        }
    }

    /// Infers the format from a path's extension.
    #[must_use]
    pub fn from_path(path: &str) -> Option<Self> {
        let ext = Path::new(path).extension()?.to_str()?;
        Self::from_name(ext)
    }

    /// A short human-readable name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::GeoJson => "GeoJSON",
            Self::Wkt => "WKT",
            Self::Wkb => "WKB",
            Self::Shapefile => "Shapefile",
            Self::GeoPackage => "GeoPackage",
        }
    }
}

/// Decodes `bytes` in `format` into features.
///
/// # Errors
/// Returns a message describing the first decode failure.
pub fn read_features(format: Format, bytes: &[u8]) -> Result<Vec<Feature>, String> {
    match format {
        Format::GeoJson => {
            let text =
                std::str::from_utf8(bytes).map_err(|e| format!("GeoJSON is not UTF-8: {e}"))?;
            parse_feature_collection(text)
                .map(|fc| fc.features)
                .map_err(|e| format!("invalid GeoJSON: {e}"))
        }
        Format::Wkt => {
            let text = std::str::from_utf8(bytes).map_err(|e| format!("WKT is not UTF-8: {e}"))?;
            text.lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .enumerate()
                .map(|(i, line)| {
                    let g = tpt_gis_io::wkt::parse_geometry(line)
                        .map_err(|e| format!("invalid WKT on line {}: {e}", i + 1))?;
                    Ok(feature(Some(g)))
                })
                .collect()
        }
        Format::Wkb => {
            let g =
                tpt_gis_io::wkb::parse_geometry(bytes).map_err(|e| format!("invalid WKB: {e}"))?;
            Ok(vec![feature(Some(g))])
        }
        Format::Shapefile => {
            let geometries = tpt_gis_io::shapefile::read_shp(bytes)
                .map_err(|e| format!("invalid shapefile: {e}"))?;
            Ok(geometries.into_iter().map(|g| feature(Some(g))).collect())
        }
        Format::GeoPackage => {
            let features = read_gpkg(bytes).map_err(|e| format!("invalid GeoPackage: {e}"))?;
            Ok(features)
        }
    }
}

/// Encodes features as `format`.
///
/// # Errors
/// Returns a message if the format cannot be written (shapefile output is not
/// supported) or the encoder rejects the data.
pub fn write_features(format: Format, features: &[Feature]) -> Result<Vec<u8>, String> {
    match format {
        Format::GeoJson => {
            let fc = FeatureCollection { features: features.to_vec() };
            Ok(write_feature_collection(&fc).into_bytes())
        }
        Format::Wkt => {
            let mut out = String::new();
            for f in features {
                if let Some(g) = &f.geometry {
                    out.push_str(&tpt_gis_io::wkt::write_geometry(g));
                    out.push('\n');
                }
            }
            Ok(out.into_bytes())
        }
        Format::Wkb => {
            let parts: Vec<Geometry> = features.iter().filter_map(|f| f.geometry.clone()).collect();
            let collection = Geometry::GeometryCollection(parts);
            Ok(tpt_gis_io::wkb::write_geometry(&collection))
        }
        Format::Shapefile => Err("writing Shapefile output is not supported yet".to_string()),
        Format::GeoPackage => {
            let opts = WriteOptions {
                table_name: "features".into(),
                geometry_column: "geom".into(),
                srs_id: 4326,
                id_column: None,
                description: None,
            };
            write_gpkg(features, &opts).map_err(|e| format!("failed to write GeoPackage: {e}"))
        }
    }
}

fn feature(geometry: Option<Geometry>) -> Feature {
    Feature { geometry, properties: Map::<String, Value>::new() }
}

/// Every coordinate in a geometry, for bounds computation.
fn collect_points(geometry: &Geometry, out: &mut Vec<Point>) {
    match geometry {
        Geometry::Point(p) => out.push(*p),
        Geometry::LineString(pts) | Geometry::MultiPoint(pts) => out.extend(pts.iter().copied()),
        Geometry::Polygon(poly) => {
            for ring in &poly.rings {
                out.extend(ring.iter().copied());
            }
        }
        Geometry::MultiLineString(lines) => {
            for line in lines {
                out.extend(line.iter().copied());
            }
        }
        Geometry::MultiPolygon(polys) => {
            for poly in polys {
                for ring in &poly.rings {
                    out.extend(ring.iter().copied());
                }
            }
        }
        Geometry::GeometryCollection(items) => {
            for item in items {
                collect_points(item, out);
            }
        }
    }
}

/// The GeoJSON-style type name of a geometry.
#[must_use]
pub fn geometry_type_name(geometry: &Geometry) -> &'static str {
    match geometry {
        Geometry::Point(_) => "Point",
        Geometry::LineString(_) => "LineString",
        Geometry::Polygon(_) => "Polygon",
        Geometry::MultiPoint(_) => "MultiPoint",
        Geometry::MultiLineString(_) => "MultiLineString",
        Geometry::MultiPolygon(_) => "MultiPolygon",
        Geometry::GeometryCollection(_) => "GeometryCollection",
    }
}

/// A summary of a dataset: format, feature counts, geometry types, and bounds.
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    /// Number of features.
    pub features: usize,
    /// Features with no geometry.
    pub without_geometry: usize,
    /// Count of each geometry type.
    pub geometry_types: BTreeMap<&'static str, usize>,
    /// Bounding box `(min_x, min_y, max_x, max_y)` of all coordinates, if any.
    pub bounds: Option<(f64, f64, f64, f64)>,
    /// Property names seen across all features, sorted.
    pub property_names: Vec<String>,
}

/// Summarises a list of features.
#[must_use]
pub fn summarize(features: &[Feature]) -> Summary {
    let mut geometry_types: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut points = Vec::new();
    let mut without_geometry = 0;
    let mut names = std::collections::BTreeSet::new();
    for f in features {
        names.extend(f.properties.keys().cloned());
        match &f.geometry {
            Some(g) => {
                *geometry_types.entry(geometry_type_name(g)).or_insert(0) += 1;
                collect_points(g, &mut points);
            }
            None => without_geometry += 1,
        }
    }
    let bounds = points.iter().fold(None, |acc: Option<(f64, f64, f64, f64)>, p| {
        Some(match acc {
            None => (p.x, p.y, p.x, p.y),
            Some((a, b, c, d)) => (a.min(p.x), b.min(p.y), c.max(p.x), d.max(p.y)),
        })
    });
    Summary {
        features: features.len(),
        without_geometry,
        geometry_types,
        bounds,
        property_names: names.into_iter().collect(),
    }
}

/// Renders a [`Summary`] as a JSON object.
#[must_use]
pub fn summary_to_json(format: Format, summary: &Summary) -> Value {
    let bounds = summary.bounds.map_or(Value::Null, |(a, b, c, d)| {
        Value::Array(vec![a.into(), b.into(), c.into(), d.into()])
    });
    let types: Map<String, Value> = summary
        .geometry_types
        .iter()
        .map(|(name, count)| ((*name).to_string(), Value::from(*count)))
        .collect();
    serde_json::json!({
        "format": format.name(),
        "features": summary.features,
        "without_geometry": summary.without_geometry,
        "geometry_types": types,
        "bounds": bounds,
        "properties": summary.property_names,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_inferred_from_extension() {
        assert_eq!(Format::from_path("a/b/data.GeoJSON"), Some(Format::GeoJson));
        assert_eq!(Format::from_path("roads.gpkg"), Some(Format::GeoPackage));
        assert_eq!(Format::from_path("noext"), None);
        assert_eq!(Format::from_path("x.tif"), None);
    }

    #[test]
    fn wkt_round_trips_through_geojson() {
        let wkt = b"POINT (1 2)\nLINESTRING (0 0, 3 4)\n";
        let features = read_features(Format::Wkt, wkt).unwrap();
        assert_eq!(features.len(), 2);
        let json = write_features(Format::GeoJson, &features).unwrap();
        let back = read_features(Format::GeoJson, &json).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].geometry, features[0].geometry);
    }

    #[test]
    fn wkt_error_names_the_line() {
        let err = read_features(Format::Wkt, b"POINT (1 2)\nnot a geometry\n").unwrap_err();
        assert!(err.contains("line 2"), "{err}");
    }

    #[test]
    fn geopackage_round_trip_preserves_geometry() {
        let features = read_features(Format::Wkt, b"POINT (5 6)\n").unwrap();
        let bytes = write_features(Format::GeoPackage, &features).unwrap();
        let back = read_features(Format::GeoPackage, &bytes).unwrap();
        assert_eq!(back[0].geometry, features[0].geometry);
    }

    #[test]
    fn shapefile_output_is_refused_clearly() {
        let err = write_features(Format::Shapefile, &[]).unwrap_err();
        assert!(err.contains("not supported"), "{err}");
    }

    #[test]
    fn summary_reports_types_and_bounds() {
        let features =
            read_features(Format::Wkt, b"POINT (1 2)\nPOINT (-3 7)\nLINESTRING (0 0, 4 1)\n")
                .unwrap();
        let s = summarize(&features);
        assert_eq!(s.features, 3);
        assert_eq!(s.geometry_types.get("Point"), Some(&2));
        assert_eq!(s.geometry_types.get("LineString"), Some(&1));
        assert_eq!(s.bounds, Some((-3.0, 0.0, 4.0, 7.0)));
    }
}
