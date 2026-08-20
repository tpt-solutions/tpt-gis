//! `tpt-gis-raster` example: decoding a (Geo)TIFF held in memory, inspecting the
//! resulting band, and validating whether the file is a Cloud-Optimized GeoTIFF.
//!
//! Run with: `cargo run --example geotiff_demo -p tpt-gis-raster`

use tpt_gis_raster::cog;
use tpt_gis_raster::geotiff::{self, testsupport};

fn main() {
    // Build an in-memory GeoTIFF (single-band, 2x2, u8) without touching disk.
    let bytes = testsupport::make_uncompressed_tiff();
    let band = geotiff::read(&bytes).expect("readable GeoTIFF");
    println!("decoded {}x{} band, kind = {:?}", band.width(), band.height(), band.kind());

    // Downcast to a concrete band and read some statistics.
    if let Some(u8_band) = band.as_u8() {
        println!("cells: {:?}", u8_band.grid().cells());
        println!("min/max: {:?}", u8_band.min_max());
    }

    // COG validation: the fixture is stripped, so it is *not* a valid COG.
    let report = cog::validate(&bytes).expect("validatable file");
    println!("COG valid? {}", report.is_valid());
    for (issue, is_error) in &report.issues {
        println!("  - {issue:?} ({})", if *is_error { "error" } else { "warning" });
    }
}
