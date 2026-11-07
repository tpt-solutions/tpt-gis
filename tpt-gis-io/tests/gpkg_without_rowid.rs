//! Reads a GeoPackage whose feature table is `WITHOUT ROWID`, written by Python's sqlite3
//! (see `tests/fixtures/generate_without_rowid.py`). The table is large enough to need
//! interior index pages and an overflow chain, and its integer primary key must come back as
//! an ordinary column rather than being treated as a rowid alias.

use std::collections::BTreeSet;
use std::path::PathBuf;

use tpt_gis_io::geopackage::read_gpkg;
use tpt_gis_io::Geometry;

const COUNT: i64 = 1500;

fn read_fixture() -> Vec<tpt_gis_io::geojson::Feature> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/without_rowid.gpkg");
    read_gpkg(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn reads_every_row_of_a_without_rowid_table() {
    let features = read_fixture();
    assert_eq!(features.len(), COUNT as usize);

    let fids: BTreeSet<i64> =
        features.iter().map(|f| f.properties["fid"].as_i64().unwrap()).collect();
    assert_eq!(fids.len(), COUNT as usize, "no duplicate or missing keys");
    assert_eq!(fids.iter().next(), Some(&1));
    assert_eq!(fids.iter().next_back(), Some(&COUNT));
}

#[test]
fn primary_key_is_an_ordinary_column_and_geometry_decodes() {
    let features = read_fixture();
    let row = features.iter().find(|f| f.properties["fid"] == 1234).unwrap();
    // The integer key is a property, not a rowid alias: it keeps its value and type.
    assert_eq!(row.properties["fid"].as_i64(), Some(1234));
    match &row.geometry {
        Some(Geometry::Point(p)) => {
            assert!((p.x - 12.34).abs() < 1e-9, "x {}", p.x);
            assert!((p.y + 24.68).abs() < 1e-9, "y {}", p.y);
        }
        other => panic!("expected a point, got {other:?}"),
    }
    // Row 1234 has a 12 000-byte name, which spills into overflow pages.
    assert_eq!(row.properties["name"].as_str().map(str::len), Some(12000));
}

#[test]
fn rows_spanning_index_pages_keep_their_values() {
    let features = read_fixture();
    // Long names (every 97th key) and short names alike must come back intact.
    let row = features.iter().find(|f| f.properties["fid"] == 97).unwrap();
    assert_eq!(row.properties["name"].as_str().map(str::len), Some("place-00097".len() + 1 + 3000));
    let plain = features.iter().find(|f| f.properties["fid"] == 500).unwrap();
    assert_eq!(plain.properties["name"].as_str(), Some("place-00500"));
}
