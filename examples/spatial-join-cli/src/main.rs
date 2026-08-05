//! MVP 1: Planetary-Scale Spatial Join Engine — CLI binary.
//!
//! Provides three subcommands:
//! - `spatial-join`: Reads points and polygons from GeoJSON files, runs an
//!   R-Tree-accelerated points-in-polygons spatial join, and writes the matches
//!   as a GeoJSON `FeatureCollection` (or CSV).
//! - `reproject`: Reads a GeoJSON `FeatureCollection` and reprojects all
//!   coordinates from WGS84 to a supported target CRS.
//! - `generate-fixtures`: Writes synthetic GeoJSON test fixtures (points and
//!   polygons) for benchmarking and development.

use clap::{Parser, Subcommand};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
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

type Payload = Map<String, Value>;
type SpatialJoinResult = JoinResult<Payload, Payload>;

/// MVP 1: Planetary-Scale Spatial Join Engine.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
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
        #[arg(long, default_value = "geojson")]
        format: String,

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

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();

    match cli.command {
        Commands::SpatialJoin { points, polygons, output, format, incremental } => {
            run_spatial_join(points, polygons, output, format, incremental)
        }
        Commands::Reproject { input, crs, output } => run_reproject(input, crs, output),
        Commands::GenerateFixtures { points, polygons, seed, output } => {
            run_generate_fixtures(points, polygons, seed, output)
        }
    }
}

fn run_spatial_join(
    points_path: String,
    polygons_path: String,
    output: Option<String>,
    format: String,
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
        .filter_map(|f| {
            f.geometry.and_then(|g| match g {
                Geometry::Point(p) => Some(IndexedPoint { point: p, payload: f.properties }),
                _ => None,
            })
        })
        .collect();

    let polygons: Vec<IndexedPolygon<Map<String, Value>>> = polygons_fc
        .features
        .into_iter()
        .filter_map(|f| {
            f.geometry.and_then(|g| match g {
                Geometry::Polygon(p) => Some(IndexedPolygon {
                    polygon: Polygon::from_exterior(p.rings[0].clone()),
                    payload: f.properties,
                }),
                _ => None,
            })
        })
        .collect();

    let config = SpatialJoinConfig { use_bulk_load: !incremental };

    let results = spatial_join(&points, &polygons, config);

    let output_text = match format.as_str() {
        "csv" => write_csv(&results)?,
        _ => write_geojson(&results),
    };

    match &output {
        Some(path) => std::fs::write(path, output_text)
            .map_err(|e| format!("failed to write output file: {e}"))?,
        None => print!("{output_text}"),
    }
    Ok(())
}

fn run_generate_fixtures(
    num_points: usize,
    num_polygons: usize,
    seed: u64,
    output_dir: String,
) -> Result<(), String> {
    let mut rng = StdRng::seed_from_u64(seed);
    std::fs::create_dir_all(&output_dir)
        .map_err(|e| format!("failed to create output directory: {e}"))?;

    let points: Vec<Feature> = (0..num_points)
        .map(|i| Feature {
            geometry: Some(Geometry::Point(Point::new(
                rng.gen_range(-1000.0..1000.0),
                rng.gen_range(-1000.0..1000.0),
            ))),
            properties: Map::from_iter([("id".to_string(), Value::Number(i.into()))]),
        })
        .collect();

    let polygons: Vec<Feature> = (0..num_polygons)
        .map(|i| {
            let cx = rng.gen_range(-900.0..900.0);
            let cy = rng.gen_range(-900.0..900.0);
            let size = rng.gen_range(10.0..100.0);
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
        None => print!("{text}"),
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
            Ok(Crs::Utm(Zone::new(num, hem)))
        }
        _ => Err("unsupported target CRS: {s} (supported: web-mercator, utm-18n, utm-33s, ...)"
            .to_string()),
    }
}

fn reproject_geometry(geometry: Geometry, target: Crs) -> Geometry {
    match geometry {
        Geometry::Point(p) => {
            let geo = GeoPoint::new(p.y, p.x);
            let projected = project_forward(geo, target);
            let PlanarPoint { x, y } = projected;
            Geometry::Point(Point::new(x, y))
        }
        Geometry::LineString(pts) => {
            let pts = pts
                .into_iter()
                .map(|p| {
                    let geo = GeoPoint::new(p.y, p.x);
                    let projected = project_forward(geo, target);
                    let PlanarPoint { x, y } = projected;
                    Point::new(x, y)
                })
                .collect();
            Geometry::LineString(pts)
        }
        Geometry::Polygon(poly) => {
            let rings = poly
                .rings
                .into_iter()
                .map(|ring| {
                    ring.into_iter()
                        .map(|p| {
                            let geo = GeoPoint::new(p.y, p.x);
                            let projected = project_forward(geo, target);
                            let PlanarPoint { x, y } = projected;
                            Point::new(x, y)
                        })
                        .collect()
                })
                .collect();
            Geometry::Polygon(Polygon { rings })
        }
        other => other,
    }
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
        let mut row = csv::StringRecord::new();
        row.push_field(&r.point.x.to_string());
        row.push_field(&r.point.y.to_string());
        for (_k, v) in &r.point_payload {
            row.push_field(&v.to_string());
        }
        wtr.write_record(&row).map_err(|e| format!("failed to write csv record: {e}"))?;
    }
    let bytes = wtr.into_inner().map_err(|e| format!("failed to finalize csv output: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("csv output is not valid utf8: {e}"))
}
