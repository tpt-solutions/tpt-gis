//! `validate`: checks a file and reports problems as JSON.
//!
//! Vector files (GeoJSON, WKT, WKB, Shapefile, GeoPackage) are checked feature by feature:
//! polygons must be valid (see `Polygon::is_valid`), and line strings need at least two
//! points. GeoTIFF files (`.tif`, `.tiff`) are checked against the Cloud-Optimized GeoTIFF
//! layout rules.

use serde_json::{json, Value};
use tpt_gis_geom::Polygon as GeomPolygon;
use tpt_gis_io::geojson::Feature;
use tpt_gis_io::geometry::{Geometry, Polygon};

use crate::convert::Format;

/// One problem found in a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// Where the problem is: a feature index, or a TIFF layout item.
    pub location: String,
    /// What is wrong.
    pub message: String,
}

fn polygon_is_valid(polygon: &Polygon) -> bool {
    let mut scratch = Vec::new();
    let view: GeomPolygon = polygon.as_geom(&mut scratch);
    view.is_valid()
}

fn check_geometry(geometry: &Geometry, location: &str, problems: &mut Vec<Problem>) {
    match geometry {
        Geometry::Polygon(p) => {
            if !polygon_is_valid(p) {
                problems.push(Problem {
                    location: location.to_string(),
                    message: "invalid polygon (rings must be closed, simple, and holes must lie inside the exterior)"
                        .to_string(),
                });
            }
        }
        Geometry::MultiPolygon(parts) => {
            for (i, p) in parts.iter().enumerate() {
                if !polygon_is_valid(p) {
                    problems.push(Problem {
                        location: format!("{location}.part[{i}]"),
                        message: "invalid polygon".to_string(),
                    });
                }
            }
        }
        Geometry::LineString(points) if points.len() < 2 => problems.push(Problem {
            location: location.to_string(),
            message: "line string has fewer than two points".to_string(),
        }),
        Geometry::MultiLineString(lines) => {
            for (i, line) in lines.iter().enumerate() {
                if line.len() < 2 {
                    problems.push(Problem {
                        location: format!("{location}.part[{i}]"),
                        message: "line string has fewer than two points".to_string(),
                    });
                }
            }
        }
        Geometry::GeometryCollection(items) => {
            for (i, item) in items.iter().enumerate() {
                check_geometry(item, &format!("{location}.item[{i}]"), problems);
            }
        }
        _ => {}
    }
}

fn check_features(features: &[Feature]) -> Vec<Problem> {
    let mut problems = Vec::new();
    for (i, feature) in features.iter().enumerate() {
        if let Some(geometry) = &feature.geometry {
            check_geometry(geometry, &format!("feature[{i}]"), &mut problems);
        }
    }
    problems
}

fn check_tiff(bytes: &[u8]) -> Vec<Problem> {
    match tpt_gis_raster::cog::validate(bytes) {
        Err(e) => {
            vec![Problem { location: "file".into(), message: format!("not a readable TIFF: {e}") }]
        }
        Ok(report) => report
            .issues
            .iter()
            .map(|(issue, is_error)| Problem {
                location: "layout".into(),
                message: format!("{}{issue:?}", if *is_error { "error: " } else { "warning: " }),
            })
            .collect(),
    }
}

/// Validates the file at `path`, returning a JSON report and the list of problems.
///
/// # Errors
/// Returns a message if the file cannot be read or its format is unsupported.
pub fn validate_file(path: &str) -> Result<(Value, Vec<Problem>), String> {
    let lower = path.to_ascii_lowercase();
    let bytes = std::fs::read(path).map_err(|e| format!("failed to read {path}: {e}"))?;
    let (format_name, problems) = if lower.ends_with(".tif") || lower.ends_with(".tiff") {
        ("GeoTIFF", check_tiff(&bytes))
    } else {
        let format = Format::from_path(path).ok_or_else(|| {
            format!("unsupported file type for '{path}' (supported: .geojson, .json, .wkt, .wkb, .shp, .gpkg, .tif)")
        })?;
        let features = crate::convert::read_file_features(path, format, &bytes)?;
        (format.name(), check_features(&features))
    };
    let report = json!({
        "file": path,
        "format": format_name,
        "valid": problems.is_empty(),
        "problems": problems
            .iter()
            .map(|p| json!({"location": p.location, "message": p.message}))
            .collect::<Vec<_>>(),
    });
    Ok((report, problems))
}
