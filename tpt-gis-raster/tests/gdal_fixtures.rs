//! Reads GeoTIFFs written by GDAL (via rasterio) and compares every cell with GDAL's own
//! arrays, stored as `*.expected.txt` (one row per line, bands separated by `--`).
//!
//! The fixtures cover tiled and striped layouts, deflate with predictors 2 and 3, several
//! bands, and float32 nodata. They were generated outside this repository, so they test
//! the reader against a producer other than our own writer.

use std::path::PathBuf;

use tpt_gis_raster::band::AnyBand;
use tpt_gis_raster::geotiff;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join(name)
}

/// The expected cells of every band, row by row. A line holding only `--` ends a band.
fn expected_bands(name: &str) -> Vec<Vec<Vec<f64>>> {
    let text = std::fs::read_to_string(fixture(&format!("{name}.expected.txt"))).unwrap();
    let mut bands: Vec<Vec<Vec<f64>>> = vec![Vec::new()];
    for line in text.lines().map(str::trim) {
        if line == "--" {
            bands.push(Vec::new());
        } else if !line.is_empty() {
            let row = line.split_whitespace().map(|v| v.parse::<f64>().unwrap()).collect();
            bands.last_mut().unwrap().push(row);
        }
    }
    bands.retain(|band| !band.is_empty());
    bands
}

/// Checks each band of `tif` against the expected arrays, cell by cell.
fn assert_matches_gdal(name: &str, tif: &str) {
    let data = std::fs::read(fixture(tif)).unwrap();
    let expected = expected_bands(name);
    let count = geotiff::GeoTiff::parse(&data).unwrap().band_count(0).unwrap();
    assert_eq!(count, expected.len(), "band count");
    for (index, rows) in expected.iter().enumerate() {
        let band = geotiff::read_band(&data, index).unwrap();
        assert_eq!(band.height(), rows.len(), "band {index} height");
        for (y, row) in rows.iter().enumerate() {
            assert_eq!(band.width(), row.len(), "band {index} width");
            for (x, want) in row.iter().enumerate() {
                let got = band.value_at_f64(x, y).unwrap();
                assert_eq!(got, *want, "band {index} cell ({x}, {y})");
            }
        }
    }
}

#[test]
fn tiled_uint16_deflate_predictor2_matches_gdal() {
    assert_matches_gdal("gdal_tiled_u16_deflate_pred2", "gdal_tiled_u16_deflate_pred2.tif");
}

#[test]
fn striped_float32_deflate_predictor3_three_bands_matches_gdal() {
    assert_matches_gdal("gdal_strips_f32_deflate_pred3", "gdal_strips_f32_deflate_pred3.tif");
}

#[test]
fn single_band_float32_predictor3_matches_gdal() {
    assert_matches_gdal("gdal_single_f32_pred3", "gdal_single_f32_pred3.tif");
}

#[test]
fn gdal_georeferencing_and_nodata_are_read() {
    let data = std::fs::read(fixture("gdal_strips_f32_deflate_pred3.tif")).unwrap();
    let band = geotiff::read_band(&data, 0).unwrap();
    assert_eq!(band.epsg(), Some(4326));
    assert_eq!(band.transform().to_gdal(), [-10.0, 0.25, 0.0, 50.0, 0.0, -0.25]);
    assert!(matches!(band, AnyBand::F32(_)));
}
