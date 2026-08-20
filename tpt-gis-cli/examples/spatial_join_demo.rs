//! `tpt-gis-cli` example: drive the spatial-join CLI end to end.
//!
//! Generates point/polygon fixtures with one subcommand, then runs the
//! `spatial-join` subcommand against them and reports the output file.
//!
//! Run with: `cargo run --example spatial_join_demo -p tpt-gis-cli`

fn main() {
    let dir = std::env::temp_dir().join("tptgis_join_demo");
    std::fs::create_dir_all(&dir).expect("create temp dir");

    let points = dir.join("points_200.geojson");
    let polygons = dir.join("polygons_20.geojson");
    let out = dir.join("join.geojson");

    // 1) Generate synthetic fixtures via the CLI's own subcommand.
    tpt_gis_cli::run_with_args([
        "tptgis",
        "generate-fixtures",
        "--points",
        "200",
        "--polygons",
        "20",
        "--seed",
        "7",
        "--output",
        dir.to_str().unwrap(),
    ])
    .expect("generate-fixtures");

    // 2) Run the spatial join and write the matches as GeoJSON.
    tpt_gis_cli::run_with_args([
        "tptgis",
        "spatial-join",
        "--points",
        points.to_str().unwrap(),
        "--polygons",
        polygons.to_str().unwrap(),
        "--format",
        "geojson",
        "--output",
        out.to_str().unwrap(),
    ])
    .expect("spatial-join");

    let written = std::fs::read_to_string(&out).expect("read join output");
    let match_count = written.matches("\"Feature\"").count();
    println!("wrote {match_count} matched feature(s) to {}", out.display());
}
