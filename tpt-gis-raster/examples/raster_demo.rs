//! `tpt-gis-raster` example: grids, map algebra, and resampling.
//!
//! Run with: `cargo run --example raster_demo -p tpt-gis-raster`

use tpt_gis_raster::{algebra, resample, Band, GeoTransform, Grid, Resampling};

fn main() {
    // --- Build a band with a nodata hole -------------------------------------
    let grid = Grid::new(3, 3, vec![1i16, 2, 3, 4, -9999, 6, 7, 8, 9]).unwrap();
    let dem = Band::new(grid, GeoTransform::new(0.0, 0.0, 1.0, 1.0)).with_nodata(-9999);

    // Local op: nodata in, nodata out — not -19998.
    let doubled = algebra::add(&dem, &dem).unwrap();
    println!("doubled centre (nodata preserved): {:?}", doubled.get(1, 1));

    // A focal mean fills the hole from its (valid) neighbours.
    let smoothed = algebra::focal_mean(&dem, 1).unwrap();
    println!("focal mean centre: {:?}", smoothed.get(1, 1));

    // --- Resample / overview building ----------------------------------------
    let cells: Vec<f32> = (0..16).map(|i| i as f32).collect();
    let band = Band::new(Grid::new(4, 4, cells).unwrap(), GeoTransform::new(0.0, 4.0, 1.0, 1.0));
    let overview = resample::downsample(&band, 2, Resampling::Average).unwrap();
    println!(
        "overview size: {}x{}, value(0,0) = {:?}",
        overview.width(),
        overview.height(),
        overview.get(0, 0)
    );
}
