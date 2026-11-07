//! MVP 1: Planetary-Scale Spatial Join Engine — CLI library.
//!
//! Subcommands (exposed through the `tptgis` binary):
//! - `spatial-join`: points-in-polygons join (holes and multi-part geometries
//!   honoured), written as GeoJSON or CSV.
//! - `reproject`: reprojects a GeoJSON file from WGS84 to a supported CRS.
//! - `convert`: converts between GeoJSON, WKT, WKB, Shapefile, and GeoPackage.
//! - `info`: prints a JSON summary of a vector file.
//! - `geofence-compile`: emits a `no_std`-friendly Rust module of polygon data.
//! - `generate-fixtures`: writes synthetic GeoJSON test fixtures for benchmarking.

use clap::{Parser, Subcommand};
use serde_json::Map;
use serde_json::Value;
use tpt_gis_core::projection::{utm::Zone, web_mercator};
use tpt_gis_core::{Crs, GeoPoint, PlanarPoint};
use tpt_gis_index::spatial_join::{
    spatial_join, IndexedPoint, IndexedPolygon, JoinResult, SpatialJoinConfig,
};
use tpt_gis_io::geojson::{
    parse_feature_collection, write_feature_collection, Feature, FeatureCollection,
};
use tpt_gis_io::geometry::{Geometry, Point, Polygon};

mod convert;
mod csv;
mod geofence;
pub mod rng;
use crate::rng::SmallRng;

type Payload = Map<String, Value>;
type SpatialJoinResult = JoinResult<Payload, Payload>;

/// MVP 1: Planetary-Scale Spatial Join Engine.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Output encodings for `spatial-join`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum OutputFormat {
    /// GeoJSON FeatureCollection (default).
    Geojson,
    /// Comma-separated values.
    Csv,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run a spatial join between points and polygons.
    SpatialJoin {
        /// GeoJSON FeatureCollection of points to test.
        #[arg(long)]
        points: String,

        /// GeoJSON FeatureCollection of polygons to test against.
        #[arg(long)]
        polygons: String,

        /// Output file path. Defaults to stdout if omitted.
        #[arg(long, short)]
        output: Option<String>,

        /// Output format: geojson (default) or csv.
        #[arg(long, value_enum, default_value_t = OutputFormat::Geojson)]
        format: OutputFormat,

        /// Use incremental R-Tree insert instead of bulk loading.
        #[arg(long)]
        incremental: bool,
    },
    /// Reproject a GeoJSON FeatureCollection from WGS84 to a target CRS.
    Reproject {
        /// Input GeoJSON FeatureCollection file.
        #[arg(long)]
        input: String,

        /// Target CRS: web-mercator or utm-zone (e.g. utm-18n, utm-33s).
        #[arg(long)]
        crs: String,

        /// Output file path. Defaults to stdout if omitted.
        #[arg(long, short)]
        output: Option<String>,
    },
    /// Convert a vector file between formats (GeoJSON, WKT, WKB, Shapefile, GeoPackage).
    Convert {
        /// Input file, or `-` to read stdin (then `--from` is required).
        #[arg(long)]
        input: String,

        /// Output file. Defaults to stdout if omitted.
        #[arg(long, short)]
        output: Option<String>,

        /// Input format (geojson, wkt, wkb, shp, gpkg). Default: from the input extension.
        #[arg(long)]
        from: Option<String>,

        /// Output format. Default: from the output extension, else GeoJSON.
        #[arg(long)]
        to: Option<String>,
    },
    /// Print a JSON summary of a vector file: feature count, geometry types, bounds,
    /// and property names.
    Info {
        /// Input file.
        input: String,
    },
    /// Compile GeoJSON polygons into a `no_std`-friendly Rust module of const data
    /// (bounding boxes, exterior rings, and holes) for embedded geofence checks.
    GeofenceCompile {
        /// GeoJSON FeatureCollection of polygons (a `name` property names each zone).
        #[arg(long)]
        input: String,

        /// Output `.rs` file. Defaults to stdout if omitted.
        #[arg(long, short)]
        output: Option<String>,
    },
    /// Generate synthetic GeoJSON test fixtures.
    GenerateFixtures {
        /// Number of points to generate.
        #[arg(long, default_value_t = 10_000)]
        points: usize,

        /// Number of polygons to generate.
        #[arg(long, default_value_t = 500)]
        polygons: usize,

        /// Random seed for reproducibility.
        #[arg(long, default_value_t = 42)]
        seed: u64,

        /// Output directory for generated files.
        #[arg(long, short)]
        output: String,
    },
}

/// Entry point for the `tptgis` CLI. Returns a process-exit error string on
/// failure (the caller is expected to print it and exit non-zero).
///
/// # Errors
/// Returns an error string if argument parsing or any subcommand fails.
pub fn cli_main() -> Result<(), String> {
    run_with_args(std::env::args_os())
}

/// Runs the CLI from an explicit set of arguments (rather than `std::env::args`),
/// so it can be driven programmatically — from integration tests, the `examples/`
/// directory, or any embedding host. The first item is treated as the program
/// name, exactly as `std::env::args` supplies it.
///
/// # Errors
/// Returns an error string if argument parsing or any subcommand fails.
pub fn run_with_args<I, T>(args: I) -> Result<(), String>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    // `try_parse_from` rather than `parse_from`: the latter exits the process on bad
    // arguments, which would make the function untestable and unsuitable for embedding.
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(e) if e.use_stderr() => {
            let message = e.to_string();
            return Err(message.strip_prefix("error: ").unwrap_or(&message).trim_end().to_string());
        }
        Err(e) => {
            // Help and version output go to stdout and count as success.
            let _ = e.print();
            return Ok(());
        }
    };
    match cli.command {
        Commands::SpatialJoin { points, polygons, output, format, incremental } => {
            run_spatial_join(points, polygons, output, format, incremental)
        }
        Commands::Reproject { input, crs, output } => run_reproject(input, crs, output),
        Commands::GenerateFixtures { points, polygons, seed, output } => {
            run_generate_fixtures(points, polygons, seed, output)
        }
        Commands::Convert { input, output, from, to } => run_convert(input, output, from, to),
        Commands::Info { input } => run_info(input),
        Commands::GeofenceCompile { input, output } => run_geofence_compile(input, output),
    }
}

/// Writes `text` to stdout, treating a closed pipe (e.g. `tptgis ... | head`) as a
/// normal end of output rather than a panic.
fn write_stdout(text: &str) -> Result<(), String> {
    write_stdout_bytes(text.as_bytes())
}

fn write_stdout_bytes(bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut out = std::io::stdout().lock();
    match out.write_all(bytes).and_then(|()| out.flush()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        Err(e) => Err(format!("failed to write to stdout: {e}")),
    }
}

/// Resolves a format from an explicit name, else from a path's extension.
fn resolve_format(
    name: Option<&str>,
    path: Option<&str>,
    what: &str,
) -> Result<convert::Format, String> {
    match (name, path) {
        (Some(n), _) => convert::Format::from_name(n).ok_or_else(|| {
            format!("unknown {what} format '{n}' (use geojson, wkt, wkb, shp, gpkg)")
        }),
        (None, Some(p)) if p != "-" => convert::Format::from_path(p).ok_or_else(|| {
            format!("cannot infer the {what} format from '{p}'; pass --{what} explicitly")
        }),
        _ => Err(format!("pass --{what} to name the {what} format")),
    }
}

fn run_convert(
    input: String,
    output: Option<String>,
    from: Option<String>,
    to: Option<String>,
) -> Result<(), String> {
    let in_format = resolve_format(from.as_deref(), Some(&input), "from")?;
    let bytes = if input == "-" {
        use std::io::Read;
        let mut buf = Vec::new();
        std::io::stdin().read_to_end(&mut buf).map_err(|e| format!("failed to read stdin: {e}"))?;
        buf
    } else {
        std::fs::read(&input).map_err(|e| format!("failed to read {input}: {e}"))?
    };
    let features = convert::read_features(in_format, &bytes)?;

    let out_format = match (to.as_deref(), output.as_deref()) {
        (None, None) => convert::Format::GeoJson,
        _ => resolve_format(to.as_deref(), output.as_deref(), "to")?,
    };
    let encoded = convert::write_features(out_format, &features)?;
    match output.as_deref() {
        Some(path) if path != "-" => {
            std::fs::write(path, &encoded).map_err(|e| format!("failed to write {path}: {e}"))
        }
        _ => write_stdout_bytes(&encoded),
    }
}

fn run_geofence_compile(input: String, output: Option<String>) -> Result<(), String> {
    let text =
        std::fs::read_to_string(&input).map_err(|e| format!("failed to read {input}: {e}"))?;
    let fc = parse_feature_collection(&text).map_err(|e| format!("invalid GeoJSON: {e}"))?;
    let source = geofence::generate(&fc.features)?;
    match output.as_deref() {
        Some(path) if path != "-" => {
            std::fs::write(path, source).map_err(|e| format!("failed to write {path}: {e}"))
        }
        _ => write_stdout(&source),
    }
}

fn run_info(input: String) -> Result<(), String> {
    let format = convert::Format::from_path(&input).ok_or_else(|| {
        format!("unsupported file type for '{input}' (supported: .geojson, .json, .wkt, .wkb, .shp, .gpkg)")
    })?;
    let bytes = std::fs::read(&input).map_err(|e| format!("failed to read {input}: {e}"))?;
    let features = convert::read_features(format, &bytes)?;
    let summary = convert::summarize(&features);
    let json = convert::summary_to_json(format, &summary);
    let text =
        serde_json::to_string_pretty(&json).map_err(|e| format!("failed to render JSON: {e}"))?;
    write_stdout(&format!("{text}\n"))
}

fn run_spatial_join(
    points_path: String,
    polygons_path: String,
    output: Option<String>,
    format: OutputFormat,
    incremental: bool,
) -> Result<(), String> {
    let points_json = std::fs::read_to_string(&points_path)
        .map_err(|e| format!("failed to read points file: {e}"))?;
    let polygons_json = std::fs::read_to_string(&polygons_path)
        .map_err(|e| format!("failed to read polygons file: {e}"))?;

    let points_fc = parse_feature_collection(&points_json)
        .map_err(|e| format!("failed to parse points FeatureCollection: {e}"))?;
    let polygons_fc = parse_feature_collection(&polygons_json)
        .map_err(|e| format!("failed to parse polygons FeatureCollection: {e}"))?;

    let points: Vec<IndexedPoint<Map<String, Value>>> = points_fc
        .features
        .into_iter()
        .flat_map(|f| {
            let coords: Vec<Point> = match f.geometry {
                Some(Geometry::Point(p)) => vec![p],
                Some(Geometry::MultiPoint(ps)) => ps,
                _ => Vec::new(),
            };
            let payload = f.properties;
            coords.into_iter().map(move |point| IndexedPoint { point, payload: payload.clone() })
        })
        .collect();

    // Holes are kept (the whole polygon is indexed, not just its exterior ring), and
    // each part of a MultiPolygon is joined separately under the feature's payload.
    let polygons: Vec<IndexedPolygon<Map<String, Value>>> = polygons_fc
        .features
        .into_iter()
        .flat_map(|f| {
            let parts: Vec<Polygon> = match f.geometry {
                Some(Geometry::Polygon(p)) => vec![p],
                Some(Geometry::MultiPolygon(ps)) => ps,
                _ => Vec::new(),
            };
            let payload = f.properties;
            parts
                .into_iter()
                .map(move |polygon| IndexedPolygon { polygon, payload: payload.clone() })
        })
        .collect();

    let config = SpatialJoinConfig { use_bulk_load: !incremental };

    let results = spatial_join(&points, &polygons, config);

    let output_text = match format {
        OutputFormat::Csv => write_csv(&results)?,
        OutputFormat::Geojson => write_geojson(&results),
    };

    match &output {
        Some(path) => std::fs::write(path, output_text)
            .map_err(|e| format!("failed to write output file: {e}"))?,
        None => write_stdout(&output_text)?,
    }
    Ok(())
}

fn run_generate_fixtures(
    num_points: usize,
    num_polygons: usize,
    seed: u64,
    output_dir: String,
) -> Result<(), String> {
    let mut rng = SmallRng::seed_from_u64(seed);
    std::fs::create_dir_all(&output_dir)
        .map_err(|e| format!("failed to create output directory: {e}"))?;

    let points: Vec<Feature> = (0..num_points)
        .map(|i| Feature {
            geometry: Some(Geometry::Point(Point::new(
                rng.gen_range(-1000.0, 1000.0),
                rng.gen_range(-1000.0, 1000.0),
            ))),
            properties: Map::from_iter([("id".to_string(), Value::Number(i.into()))]),
        })
        .collect();

    let polygons: Vec<Feature> = (0..num_polygons)
        .map(|i| {
            let cx = rng.gen_range(-900.0, 900.0);
            let cy = rng.gen_range(-900.0, 900.0);
            let size = rng.gen_range(10.0, 100.0);
            let ring = vec![
                Point::new(cx - size, cy - size),
                Point::new(cx + size, cy - size),
                Point::new(cx + size, cy + size),
                Point::new(cx - size, cy + size),
                Point::new(cx - size, cy - size),
            ];
            Feature {
                geometry: Some(Geometry::Polygon(Polygon::from_exterior(ring))),
                properties: Map::from_iter([("id".to_string(), Value::Number(i.into()))]),
            }
        })
        .collect();

    let points_fc = FeatureCollection { features: points };
    let polygons_fc = FeatureCollection { features: polygons };

    let points_path = format!("{}/points_{num_points}.geojson", output_dir);
    let polygons_path = format!("{}/polygons_{num_polygons}.geojson", output_dir);

    std::fs::write(&points_path, write_feature_collection(&points_fc))
        .map_err(|e| format!("failed to write points file: {e}"))?;
    std::fs::write(&polygons_path, write_feature_collection(&polygons_fc))
        .map_err(|e| format!("failed to write polygons file: {e}"))?;

    eprintln!("Wrote {points_path}");
    eprintln!("Wrote {polygons_path}");
    Ok(())
}

fn run_reproject(input_path: String, crs: String, output: Option<String>) -> Result<(), String> {
    let json = std::fs::read_to_string(&input_path)
        .map_err(|e| format!("failed to read input file: {e}"))?;
    let fc = parse_feature_collection(&json)
        .map_err(|e| format!("failed to parse FeatureCollection: {e}"))?;

    let target = parse_crs(&crs)?;

    let features: Vec<Feature> = fc
        .features
        .into_iter()
        .map(|f| {
            let geometry = f.geometry.map(|g| reproject_geometry(g, target));
            Feature { geometry, properties: f.properties }
        })
        .collect();

    let out_fc = FeatureCollection { features };
    let text = write_feature_collection(&out_fc);

    match &output {
        Some(path) => {
            std::fs::write(path, text).map_err(|e| format!("failed to write output file: {e}"))?
        }
        None => write_stdout(&text)?,
    }
    Ok(())
}

fn parse_crs(s: &str) -> Result<Crs, String> {
    match s.to_lowercase().as_str() {
        "web-mercator" => Ok(Crs::WebMercator),
        utm if utm.starts_with("utm-") => {
            let rest = &utm[4..];
            let (num, hem) = if let Some(n) = rest.strip_suffix("n") {
                (n.parse::<u8>().map_err(|_| "invalid UTM zone number".to_string())?, true)
            } else if let Some(n) = rest.strip_suffix("s") {
                (n.parse::<u8>().map_err(|_| "invalid UTM zone number".to_string())?, false)
            } else {
                return Err(
                    "invalid UTM zone; append 'n' or 's' (e.g. utm-18n, utm-33s)".to_string()
                );
            };
            let zone = Zone::try_new(num, hem)
                .ok_or_else(|| format!("UTM zone must be between 1 and 60, got {num}"))?;
            Ok(Crs::Utm(zone))
        }
        _ => Err(format!(
            "unsupported target CRS: {s} (supported: web-mercator, utm-18n, utm-33s, ...)"
        )),
    }
}

/// Projects one WGS84 `(lon, lat)` point (stored as `x = lon`, `y = lat`) into `target`.
fn project_point(p: Point, target: Crs) -> Point {
    let projected = project_forward(GeoPoint::new(p.y, p.x), target);
    let PlanarPoint { x, y } = projected;
    Point::new(x, y)
}

fn project_points(points: Vec<Point>, target: Crs) -> Vec<Point> {
    points.into_iter().map(|p| project_point(p, target)).collect()
}

/// Reprojects every coordinate of `geometry`, including each part of multi-part
/// geometries and nested collections.
fn reproject_geometry(geometry: Geometry, target: Crs) -> Geometry {
    match geometry {
        Geometry::Point(p) => Geometry::Point(project_point(p, target)),
        Geometry::LineString(pts) => Geometry::LineString(project_points(pts, target)),
        Geometry::Polygon(poly) => Geometry::Polygon(reproject_polygon(poly, target)),
        Geometry::MultiPoint(pts) => Geometry::MultiPoint(project_points(pts, target)),
        Geometry::MultiLineString(lines) => Geometry::MultiLineString(
            lines.into_iter().map(|l| project_points(l, target)).collect(),
        ),
        Geometry::MultiPolygon(polys) => Geometry::MultiPolygon(
            polys.into_iter().map(|p| reproject_polygon(p, target)).collect(),
        ),
        Geometry::GeometryCollection(items) => Geometry::GeometryCollection(
            items.into_iter().map(|g| reproject_geometry(g, target)).collect(),
        ),
    }
}

fn reproject_polygon(poly: Polygon, target: Crs) -> Polygon {
    Polygon { rings: poly.rings.into_iter().map(|ring| project_points(ring, target)).collect() }
}

fn project_forward(geo: GeoPoint, target: Crs) -> PlanarPoint {
    match target {
        Crs::WebMercator => web_mercator::forward(geo),
        Crs::Utm(zone) => {
            tpt_gis_core::projection::utm::forward(&tpt_gis_core::Ellipsoid::WGS84, geo, zone)
        }
        Crs::Wgs84 => PlanarPoint::new(geo.lon_deg, geo.lat_deg),
    }
}

fn write_geojson(results: &[SpatialJoinResult]) -> String {
    let features: Vec<Feature> = results
        .iter()
        .map(|r| {
            let mut props = r.point_payload.clone();
            props.insert("polygon_props".to_string(), Value::Object(r.polygon_payload.clone()));
            Feature { geometry: Some(Geometry::Point(r.point)), properties: props }
        })
        .collect();
    let fc = FeatureCollection { features };
    write_feature_collection(&fc)
}

fn write_csv(results: &[SpatialJoinResult]) -> Result<String, String> {
    let mut wtr = csv::Writer::from_writer(vec![]);
    for r in results {
        let mut fields: Vec<String> = Vec::new();
        fields.push(r.point.x.to_string());
        fields.push(r.point.y.to_string());
        for (_k, v) in &r.point_payload {
            fields.push(v.to_string());
        }
        let strs: Vec<&str> = fields.iter().map(String::as_str).collect();
        wtr.write_record(strs).map_err(|e| format!("failed to write csv record: {e}"))?;
    }
    let bytes = wtr.into_inner().map_err(|e| format!("failed to finalize csv output: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("csv output is not valid utf8: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_crs_accepts_valid_utm_zones() {
        assert!(matches!(parse_crs("utm-18n"), Ok(Crs::Utm(_))));
        assert!(matches!(parse_crs("utm-33s"), Ok(Crs::Utm(_))));
    }

    /// Zone 61 used to be accepted unchecked; it must be an error naming the zone.
    #[test]
    fn parse_crs_rejects_out_of_range_zone() {
        let err = parse_crs("utm-61n").unwrap_err();
        assert!(err.contains("61"), "{err}");
    }

    /// The unsupported-CRS message must interpolate the offending value.
    /// Multi-part geometries used to pass through unprojected.
    #[test]
    fn reproject_projects_every_part_of_multi_geometries() {
        let input = Geometry::MultiPoint(vec![Point::new(0.0, 0.0), Point::new(10.0, 5.0)]);
        let out = reproject_geometry(input.clone(), Crs::WebMercator);
        match out {
            Geometry::MultiPoint(pts) => {
                assert_eq!(pts.len(), 2);
                assert!(pts[1].x.abs() > 1.0e5, "x not projected: {:?}", pts[1]);
            }
            other => panic!("unexpected {other:?}"),
        }
        assert_ne!(reproject_geometry(input, Crs::WebMercator), Geometry::MultiPoint(vec![]));
    }

    #[test]
    fn parse_crs_error_names_the_input() {
        let err = parse_crs("epsg-9999").unwrap_err();
        assert!(err.contains("epsg-9999"), "{err}");
    }
}
