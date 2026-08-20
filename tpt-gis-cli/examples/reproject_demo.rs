//! `tpt-gis-cli` example: reproject a GeoJSON FeatureCollection via the CLI.
//!
//! Writes a small GeoJSON file, reprojects its coordinates to Web Mercator
//! (EPSG:3857) with the `reproject` subcommand, and prints the result.
//!
//! Run with: `cargo run --example reproject_demo -p tpt-gis-cli`

fn main() {
    let dir = std::env::temp_dir().join("tptgis_reproject_demo");
    std::fs::create_dir_all(&dir).expect("create temp dir");

    let input = dir.join("pts.geojson");
    let out = dir.join("pts_webmercator.geojson");

    let geojson = r#"{
        "type":"FeatureCollection",
        "features":[
            {"type":"Feature","geometry":{"type":"Point","coordinates":[144.9631,-37.8136]},"properties":{"name":"Melbourne"}}
        ]
    }"#;
    std::fs::write(&input, geojson).expect("write input");

    tpt_gis_cli::run_with_args([
        "tptgis",
        "reproject",
        "--input",
        input.to_str().unwrap(),
        "--crs",
        "web-mercator",
        "--output",
        out.to_str().unwrap(),
    ])
    .expect("reproject");

    let reprojected = std::fs::read_to_string(&out).expect("read output");
    println!("reprojected GeoJSON:\n{reprojected}");
}
