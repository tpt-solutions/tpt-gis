//! Reads a GeoPackage written by GDAL (via pyogrio), covering a point, a polygon with a
//! hole, an empty point, and a line string. GDAL stores the first three with envelopes or
//! flags that our writer never produces, so this checks the blob header decoder against a
//! different producer.

use std::path::PathBuf;

use tpt_gis_io::geopackage::read_gpkg;
use tpt_gis_io::{Geometry, Point};

#[test]
fn reads_gdal_written_blobs_including_envelopes_and_empties() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/gdal_blobs.gpkg");
    let features = read_gpkg(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(features.len(), 4);

    assert_eq!(features[0].geometry, Some(Geometry::Point(Point::new(1.5, 2.5))));
    match &features[1].geometry {
        Some(Geometry::Polygon(poly)) => {
            assert_eq!(poly.interiors().len(), 1, "one hole");
            assert_eq!(poly.exterior().len(), 5, "closed square exterior");
        }
        other => panic!("expected a polygon, got {other:?}"),
    }
    // An empty point is read as no geometry; the model has no empty geometries.
    assert_eq!(features[2].geometry, None);
    assert!(matches!(features[3].geometry, Some(Geometry::LineString(_))));
}
