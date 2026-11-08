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
    let input = dir.join("data.xyz");
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

#[test]
fn validate_accepts_a_valid_polygon() {
    let dir = scratch("validate-ok");
    let input = dir.join("square.geojson");
    fs::write(
        &input,
        r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{},
            "geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1],[0,0]]]}}]}"#,
    )
    .unwrap();
    run_with_args(args(&["validate", &path_str(&input)])).expect("valid file passes");
}

/// A bow-tie ring crosses itself; validate must report it and exit with an error.
#[test]
fn validate_reports_a_self_intersecting_polygon() {
    let dir = scratch("validate-bad");
    let input = dir.join("bowtie.geojson");
    fs::write(
        &input,
        r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{},
            "geometry":{"type":"Polygon","coordinates":[[[0,0],[4,4],[4,0],[0,4],[0,0]]]}}]}"#,
    )
    .unwrap();
    let err = run_with_args(args(&["validate", &path_str(&input)])).unwrap_err();
    assert!(err.contains("1 problem"), "{err}");
}

#[test]
fn validate_reports_unreadable_tiff() {
    let dir = scratch("validate-tif");
    let input = dir.join("bogus.tif");
    fs::write(&input, b"definitely not a tiff").unwrap();
    let err = run_with_args(args(&["validate", &path_str(&input)])).unwrap_err();
    assert!(err.contains("problem"), "{err}");
}

#[test]
fn info_and_validate_accept_a_tiled_geotiff() {
    let dir = scratch("tiff");
    let input = dir.join("tiled.tif");
    let bytes = tpt_gis_raster::geotiff::testsupport::make_cog(512, 512, 256, 256, |c, r| {
        ((c + r) % 200) as u8
    });
    fs::write(&input, bytes).unwrap();
    run_with_args(args(&["info", &path_str(&input)])).expect("info succeeds");
    run_with_args(args(&["validate", &path_str(&input)])).expect("tiled COG is valid");
}

#[test]
fn reproject_accepts_an_epsg_code() {
    let dir = scratch("epsg");
    let input = dir.join("pts.geojson");
    let output = dir.join("out.geojson");
    fs::write(
        &input,
        r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{},
            "geometry":{"type":"Point","coordinates":[10,20]}}]}"#,
    )
    .unwrap();
    run_with_args(args(&[
        "reproject",
        "--input",
        &path_str(&input),
        "--crs",
        "EPSG:3857",
        "--output",
        &path_str(&output),
    ]))
    .expect("reproject succeeds");
    let text = fs::read_to_string(&output).unwrap();
    assert!(text.contains("1113194"), "{text}");
}

/// Writes a point shapefile (`.shp`) with the given points, in the layout the reader reads.
fn point_shapefile(points: &[(f64, f64)]) -> Vec<u8> {
    let content_bytes = 20; // shape type + x + y
    let file_words = (100 + points.len() * (8 + content_bytes)) / 2;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&9994i32.to_be_bytes());
    bytes.extend_from_slice(&[0u8; 20]);
    bytes.extend_from_slice(&(file_words as i32).to_be_bytes());
    bytes.extend_from_slice(&1000i32.to_le_bytes());
    bytes.extend_from_slice(&1i32.to_le_bytes());
    bytes.extend_from_slice(&[0u8; 64]);
    for (i, (x, y)) in points.iter().enumerate() {
        bytes.extend_from_slice(&((i + 1) as i32).to_be_bytes());
        bytes.extend_from_slice(&((content_bytes / 2) as i32).to_be_bytes());
        bytes.extend_from_slice(&1i32.to_le_bytes());
        bytes.extend_from_slice(&x.to_le_bytes());
        bytes.extend_from_slice(&y.to_le_bytes());
    }
    bytes
}

/// Writes a dBASE file with one character field `NAME` (16 bytes), one row per value.
fn name_dbase(names: &[&[u8]]) -> Vec<u8> {
    let field_len = 16usize;
    let header_len = 32 + 32 + 1;
    let record_len = 1 + field_len;
    let mut bytes = vec![0x03, 0, 0, 0];
    bytes.extend_from_slice(&(names.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&(header_len as u16).to_le_bytes());
    bytes.extend_from_slice(&(record_len as u16).to_le_bytes());
    bytes.extend_from_slice(&[0u8; 20]);
    let mut name = [0u8; 11];
    name[..4].copy_from_slice(b"NAME");
    bytes.extend_from_slice(&name);
    bytes.push(b'C');
    bytes.extend_from_slice(&[0u8; 4]);
    bytes.push(field_len as u8);
    bytes.push(0);
    bytes.extend_from_slice(&[0u8; 14]);
    bytes.push(0x0D);
    for value in names {
        bytes.push(b' ');
        let mut field = vec![b' '; field_len];
        field[..value.len()].copy_from_slice(value);
        bytes.extend_from_slice(&field);
    }
    bytes
}

/// Attributes come from the sibling `.dbf`, decoded with the `.cpg` encoding, and are
/// paired with the shapes by row.
#[test]
fn convert_carries_dbf_attributes_decoded_with_cpg() {
    let dir = scratch("shp-attrs");
    let shp = dir.join("cities.shp");
    let out = dir.join("cities.geojson");
    fs::write(&shp, point_shapefile(&[(1.0, 2.0), (3.0, 4.0)])).unwrap();
    // "Zoë" in Windows-1252: ë is 0xEB. The second row is plain ASCII.
    fs::write(dir.join("cities.dbf"), name_dbase(&[b"Zo\xEB", b"Oslo"])).unwrap();
    fs::write(dir.join("cities.cpg"), "CP1252\n").unwrap();

    run_with_args(args(&["convert", "--input", &path_str(&shp), "--output", &path_str(&out)]))
        .expect("convert succeeds");

    let json: serde_json::Value = serde_json::from_str(&fs::read_to_string(&out).unwrap()).unwrap();
    let features = json["features"].as_array().unwrap();
    assert_eq!(features[0]["properties"]["NAME"], "Zo\u{eb}");
    assert_eq!(features[1]["properties"]["NAME"], "Oslo");
}

#[test]
fn mismatched_dbf_row_count_is_an_error() {
    let dir = scratch("shp-mismatch");
    let shp = dir.join("cities.shp");
    fs::write(&shp, point_shapefile(&[(1.0, 2.0), (3.0, 4.0)])).unwrap();
    fs::write(dir.join("cities.dbf"), name_dbase(&[b"only one"])).unwrap();
    let err = run_with_args(args(&["info", &path_str(&shp)])).unwrap_err();
    assert!(err.contains("1 records") && err.contains("2"), "{err}");
}

/// GeoJSON → shapefile → GeoJSON keeps the geometry and the attributes (text, numbers, and
/// logicals), and writes the sidecar files.
#[test]
fn shapefile_round_trip_keeps_attributes() {
    let dir = scratch("shp-write");
    let input = dir.join("cities.geojson");
    let shp = dir.join("cities.shp");
    let back = dir.join("back.geojson");
    fs::write(
        &input,
        r#"{"type":"FeatureCollection","features":[
            {"type":"Feature","properties":{"name":"Zoë","pop":1234.5,"capital":true},
             "geometry":{"type":"Point","coordinates":[1,2]}},
            {"type":"Feature","properties":{"name":"Oslo","pop":700000,"capital":false},
             "geometry":{"type":"Point","coordinates":[3,4]}}]}"#,
    )
    .unwrap();

    run_with_args(args(&["convert", "--input", &path_str(&input), "--output", &path_str(&shp)]))
        .expect("write shapefile");
    for ext in ["shp", "shx", "dbf", "cpg"] {
        assert!(dir.join(format!("cities.{ext}")).exists(), "missing .{ext}");
    }
    run_with_args(args(&["convert", "--input", &path_str(&shp), "--output", &path_str(&back)]))
        .expect("read shapefile back");

    let json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&back).unwrap()).unwrap();
    let features = json["features"].as_array().unwrap();
    assert_eq!(features.len(), 2);
    assert_eq!(features[0]["properties"]["name"], "Zoë");
    assert_eq!(features[0]["properties"]["pop"], 1234.5);
    assert_eq!(features[0]["properties"]["capital"], true);
    assert_eq!(features[1]["properties"]["pop"], 700000.0);
    assert_eq!(features[1]["properties"]["capital"], false);
    assert_eq!(features[1]["geometry"]["coordinates"], serde_json::json!([3.0, 4.0]));
}

/// Mixed geometry types cannot share one shapefile; the error says so.
#[test]
fn shapefile_output_refuses_mixed_geometry_types() {
    let dir = scratch("shp-mixed");
    let input = dir.join("mixed.wkt");
    fs::write(&input, "POINT (1 2)\nLINESTRING (0 0, 1 1)\n").unwrap();
    let err = run_with_args(args(&[
        "convert",
        "--input",
        &path_str(&input),
        "--output",
        &path_str(&dir.join("mixed.shp")),
    ]))
    .unwrap_err();
    assert!(err.contains("mix shape types"), "{err}");
}

/// GeoJSON output gets a WGS84 .prj; a shapefile-to-shapefile conversion carries the
/// source's .prj over unchanged.
#[test]
fn shapefile_projection_sidecars() {
    let dir = scratch("prj-out");
    let input = dir.join("pts.geojson");
    let shp = dir.join("pts.shp");
    fs::write(
        &input,
        r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{},
            "geometry":{"type":"Point","coordinates":[1,2]}}]}"#,
    )
    .unwrap();
    run_with_args(args(&["convert", "--input", &path_str(&input), "--output", &path_str(&shp)]))
        .expect("write shapefile");
    let prj = fs::read_to_string(dir.join("pts.prj")).unwrap();
    assert!(prj.contains("GCS_WGS_1984"), "{prj}");

    let custom = "PROJCS[\"custom\"]";
    fs::write(dir.join("pts.prj"), custom).unwrap();
    let copy = dir.join("copy.shp");
    run_with_args(args(&["convert", "--input", &path_str(&shp), "--output", &path_str(&copy)]))
        .expect("copy shapefile");
    assert_eq!(fs::read_to_string(dir.join("copy.prj")).unwrap(), custom);
}

#[test]
fn convert_to_geopackage_with_spatial_index_writes_the_rtree() {
    let dir = scratch("gpkg_index");
    let wkt = dir.join("pts.wkt");
    std::fs::write(&wkt, "POINT (1 2)\nPOINT (3 4)\n").unwrap();
    let gpkg = dir.join("indexed.gpkg");
    run_with_args(args(&[
        "convert",
        "--input",
        &path_str(&wkt),
        "--output",
        &path_str(&gpkg),
        "--spatial-index",
    ]))
    .expect("write indexed gpkg");
    let bytes = std::fs::read(&gpkg).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("CREATE VIRTUAL TABLE"), "no rtree table in the file");
    assert!(text.contains("gpkg_rtree_index"), "no extension row in the file");
}

#[test]
fn spatial_index_is_refused_for_other_formats() {
    let dir = scratch("gpkg_index_refused");
    let wkt = dir.join("pt.wkt");
    std::fs::write(&wkt, "POINT (1 2)\n").unwrap();
    let out = dir.join("pt.geojson");
    let err = run_with_args(args(&[
        "convert",
        "--input",
        &path_str(&wkt),
        "--output",
        &path_str(&out),
        "--spatial-index",
    ]))
    .unwrap_err();
    assert!(err.contains("GeoPackage"), "{err}");
}

#[test]
fn spatial_join_csv_has_a_header_and_unquoted_strings() {
    let dir = scratch("join-csv-header");
    let points = dir.join("points.geojson");
    let polygons = dir.join("polygons.geojson");
    let output = dir.join("matches.csv");
    fs::write(
        &points,
        r#"{"type":"FeatureCollection","features":[
            {"type":"Feature","properties":{"name":"plain","rank":3},"geometry":{"type":"Point","coordinates":[0.5,0.5]}}]}"#,
    )
    .unwrap();
    fs::write(
        &polygons,
        r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{},
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
    let mut lines = csv.lines();
    assert_eq!(lines.next(), Some("x,y,name,rank"));
    assert_eq!(lines.next(), Some("0.5,0.5,plain,3"));
    assert_eq!(lines.next(), None);
}

#[test]
fn info_error_names_the_file() {
    let dir = scratch("info-path");
    let input = dir.join("bad.geojson");
    fs::write(&input, "{not json").unwrap();
    let err = run_with_args(args(&["info", &path_str(&input)])).unwrap_err();
    assert!(err.starts_with(&path_str(&input)), "{err}");
}

#[test]
fn info_reads_stdin_only_with_a_format() {
    let err = run_with_args(args(&["info", "-"])).unwrap_err();
    assert!(err.contains("--from"), "{err}");
}

#[test]
fn verbose_flag_is_accepted_globally() {
    let dir = scratch("verbose");
    let input = dir.join("in.wkt");
    let output = dir.join("out.geojson");
    fs::write(&input, "POINT (1 2)\n").unwrap();
    run_with_args(args(&[
        "--verbose",
        "convert",
        "--input",
        &path_str(&input),
        "--output",
        &path_str(&output),
    ]))
    .expect("convert succeeds with --verbose");
    run_with_args(args(&[
        "convert",
        "-v",
        "--input",
        &path_str(&input),
        "--output",
        &path_str(&output),
    ]))
    .expect("short -v also works");
}
