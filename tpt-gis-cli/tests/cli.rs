//! End-to-end tests for the `tptgis` subcommands, driven through `run_with_args` on real
//! files in a per-test temporary directory.

use std::fs;
use std::path::{Path, PathBuf};

use tpt_gis_cli::run_with_args;

/// A fresh, empty directory unique to one test.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tptgis-cli-test-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

fn args(list: &[&str]) -> Vec<String> {
    std::iter::once("tptgis").chain(list.iter().copied()).map(str::to_string).collect()
}

fn path_str(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

#[test]
fn convert_wkt_to_geojson_writes_a_feature_collection() {
    let dir = scratch("convert");
    let input = dir.join("in.wkt");
    let output = dir.join("out.geojson");
    fs::write(&input, "POINT (1 2)\nLINESTRING (0 0, 3 4)\n").unwrap();

    run_with_args(args(&["convert", "--input", &path_str(&input), "--output", &path_str(&output)]))
        .expect("convert succeeds");

    let text = fs::read_to_string(&output).unwrap();
    assert!(text.contains("FeatureCollection"), "{text}");
    assert!(text.contains("LineString"), "{text}");
}

#[test]
fn convert_round_trips_through_geopackage() {
    let dir = scratch("gpkg");
    let wkt = dir.join("in.wkt");
    let gpkg = dir.join("mid.gpkg");
    let back = dir.join("back.wkt");
    fs::write(&wkt, "POINT (5 6)\n").unwrap();

    run_with_args(args(&["convert", "--input", &path_str(&wkt), "--output", &path_str(&gpkg)]))
        .expect("write gpkg");
    run_with_args(args(&["convert", "--input", &path_str(&gpkg), "--output", &path_str(&back)]))
        .expect("read gpkg");

    assert_eq!(fs::read_to_string(&back).unwrap().trim(), "POINT (5 6)");
}

#[test]
fn convert_refuses_unknown_formats_with_a_clear_error() {
    let dir = scratch("unknown");
    let input = dir.join("data.tif");
    fs::write(&input, b"not a vector file").unwrap();
    let err = run_with_args(args(&["info", &path_str(&input)])).unwrap_err();
    assert!(err.contains("unsupported file type"), "{err}");
}

#[test]
fn info_reports_an_error_for_a_corrupt_file() {
    let dir = scratch("corrupt");
    let input = dir.join("bad.wkt");
    fs::write(&input, "POINT (1 2)\nnot geometry\n").unwrap();
    let err = run_with_args(args(&["info", &path_str(&input)])).unwrap_err();
    assert!(err.contains("line 2"), "{err}");
}

#[test]
fn geofence_compile_writes_a_rust_module() {
    let dir = scratch("fence");
    let input = dir.join("zones.geojson");
    let output = dir.join("fence.rs");
    fs::write(
        &input,
        r#"{"type":"FeatureCollection","features":[{"type":"Feature",
            "properties":{"name":"zone a"},
            "geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1],[0,0]]]}}]}"#,
    )
    .unwrap();

    run_with_args(args(&[
        "geofence-compile",
        "--input",
        &path_str(&input),
        "--output",
        &path_str(&output),
    ]))
    .expect("compile succeeds");

    let source = fs::read_to_string(&output).unwrap();
    assert!(source.contains("pub const GEOFENCES: [Geofence; 1]"), "{source}");
    assert!(source.contains("name: \"zone a\""), "{source}");
}

#[test]
fn spatial_join_writes_csv_matches() {
    let dir = scratch("join");
    let points = dir.join("points.geojson");
    let polygons = dir.join("polygons.geojson");
    let output = dir.join("matches.csv");
    fs::write(
        &points,
        r#"{"type":"FeatureCollection","features":[
            {"type":"Feature","properties":{"id":"inside"},"geometry":{"type":"Point","coordinates":[0.5,0.5]}},
            {"type":"Feature","properties":{"id":"outside"},"geometry":{"type":"Point","coordinates":[5,5]}}]}"#,
    )
    .unwrap();
    fs::write(
        &polygons,
        r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"zone":"z"},
            "geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1],[0,0]]]}}]}"#,
    )
    .unwrap();

    run_with_args(args(&[
        "spatial-join",
        "--points",
        &path_str(&points),
        "--polygons",
        &path_str(&polygons),
        "--format",
        "csv",
        "--output",
        &path_str(&output),
    ]))
    .expect("join succeeds");

    let csv = fs::read_to_string(&output).unwrap();
    assert!(csv.contains("inside"), "{csv}");
    assert!(!csv.contains("outside"), "{csv}");
}

#[test]
fn spatial_join_respects_polygon_holes() {
    let dir = scratch("holes");
    let points = dir.join("points.geojson");
    let polygons = dir.join("polygons.geojson");
    let output = dir.join("matches.csv");
    fs::write(
        &points,
        r#"{"type":"FeatureCollection","features":[
            {"type":"Feature","properties":{"id":"in-hole"},"geometry":{"type":"Point","coordinates":[5,5]}},
            {"type":"Feature","properties":{"id":"in-ring"},"geometry":{"type":"Point","coordinates":[1,1]}}]}"#,
    )
    .unwrap();
    fs::write(
        &polygons,
        r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"zone":"z"},
            "geometry":{"type":"Polygon","coordinates":[
              [[0,0],[10,0],[10,10],[0,10],[0,0]],
              [[4,4],[6,4],[6,6],[4,6],[4,4]]]}}]}"#,
    )
    .unwrap();

    run_with_args(args(&[
        "spatial-join",
        "--points",
        &path_str(&points),
        "--polygons",
        &path_str(&polygons),
        "--format",
        "csv",
        "--output",
        &path_str(&output),
    ]))
    .expect("join succeeds");

    let csv = fs::read_to_string(&output).unwrap();
    assert!(csv.contains("in-ring"), "{csv}");
    assert!(!csv.contains("in-hole"), "{csv}");
}

#[test]
fn unknown_format_name_is_rejected_by_the_parser() {
    assert!(run_with_args(args(&[
        "spatial-join",
        "--format",
        "xml",
        "--points",
        "a",
        "--polygons",
        "b"
    ]))
    .is_err());
}
