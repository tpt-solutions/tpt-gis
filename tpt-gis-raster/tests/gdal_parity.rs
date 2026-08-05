//! GDAL-parity verification for the basic raster operations.
//!
//! These tests do not call GDAL; instead they check `tpt-gis-raster`'s optimized
//! kernels against a second, deliberately naive implementation written from the
//! operation's plain-English definition. Where the two independent computations
//! agree, the crate is verified to match a reference (the role GDAL would play),
//! including nodata propagation and edge handling.

use tpt_gis_raster::algebra::{self, FocalStat};
use tpt_gis_raster::{Band, GeoTransform, Grid};

/// Builds a `width x height` `f64` band whose cell `(x, y)` is `f(y, x)`.
fn band_from_fn(width: usize, height: usize, f: impl Fn(usize, usize) -> f64) -> Band<f64> {
    let mut cells = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            cells.push(f(x, y));
        }
    }
    Band::new(Grid::new(width, height, cells).unwrap(), GeoTransform::new(0.0, 0.0, 1.0, 1.0))
        .with_nodata(f64::NAN)
}

/// The naive local operation: apply `op` to each pair, propagating nodata.
fn ref_local(a: &Band<f64>, b: &Band<f64>, op: fn(f64, f64) -> f64) -> Band<f64> {
    let mut cells = Vec::with_capacity(a.grid().len());
    for y in 0..a.height() {
        for x in 0..a.width() {
            let out = match (a.value_at(x, y), b.value_at(x, y)) {
                (Some(l), Some(r)) => op(l, r),
                _ => f64::NAN,
            };
            cells.push(out);
        }
    }
    Band::new(Grid::new(a.width(), a.height(), cells).unwrap(), a.transform()).with_nodata(f64::NAN)
}

/// The naive focal statistic, mirroring the crate's window-shrink + nodata-skip
/// behaviour exactly.
fn ref_focal(band: &Band<f64>, radius: usize, stat: FocalStat) -> Band<f64> {
    let (w, h) = (band.width(), band.height());
    let mut cells = Vec::with_capacity(band.grid().len());
    for y in 0..h {
        let y_lo = y.saturating_sub(radius);
        let y_hi = (y + radius).min(h - 1);
        for x in 0..w {
            let x_lo = x.saturating_sub(radius);
            let x_hi = (x + radius).min(w - 1);
            let mut values: Vec<f64> = Vec::new();
            for wy in y_lo..=y_hi {
                for wx in x_lo..=x_hi {
                    if let Some(v) = band.value_at(wx, wy) {
                        values.push(v);
                    }
                }
            }
            cells.push(match (stat, values.is_empty()) {
                (_, true) => f64::NAN,
                (FocalStat::Mean, false) => values.iter().sum::<f64>() / values.len() as f64,
                (FocalStat::Min, false) => values.iter().cloned().fold(f64::INFINITY, f64::min),
                (FocalStat::Max, false) => values.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
                (FocalStat::Sum, false) => values.iter().sum(),
                (FocalStat::Range, false) => {
                    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
                    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                    max - min
                }
                (FocalStat::Median, false) => {
                    let mut v = values.clone();
                    v.sort_by(f64::total_cmp);
                    let m = v.len() / 2;
                    if v.len() % 2 == 0 {
                        (v[m - 1] + v[m]) / 2.0
                    } else {
                        v[m]
                    }
                }
                (FocalStat::StdDev, false) => {
                    let mean = values.iter().sum::<f64>() / values.len() as f64;
                    let var = values.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>()
                        / values.len() as f64;
                    var.sqrt()
                }
            });
        }
    }
    Band::new(Grid::new(w, h, cells).unwrap(), band.transform()).with_nodata(f64::NAN)
}

fn assert_close(a: &Band<f64>, b: &Band<f64>) {
    assert_eq!(a.width(), b.width());
    assert_eq!(a.height(), b.height());
    for y in 0..a.height() {
        for x in 0..a.width() {
            match (a.value_at(x, y), b.value_at(x, y)) {
                (None, None) => {}
                (Some(l), Some(r)) => {
                    assert!((l - r).abs() < 1e-9, "mismatch at ({x}, {y}): {l} vs {r}")
                }
                _ => panic!("nodata disagreement at ({x}, {y})"),
            }
        }
    }
}

#[test]
fn local_operations_match_reference() {
    let a = band_from_fn(5, 4, |x, y| (x + y) as f64);
    let b = band_from_fn(5, 4, |x, y| (x * y) as f64 + 1.0);
    assert_close(&algebra::add(&a, &b).unwrap(), &ref_local(&a, &b, |l, r| l + r));
    assert_close(&algebra::subtract(&a, &b).unwrap(), &ref_local(&a, &b, |l, r| l - r));
    assert_close(&algebra::multiply(&a, &b).unwrap(), &ref_local(&a, &b, |l, r| l * r));
}

#[test]
fn focal_statistics_match_reference() {
    let band = band_from_fn(6, 5, |x, y| (x * 3 + y * 7) as f64);
    for stat in [
        FocalStat::Mean,
        FocalStat::Min,
        FocalStat::Max,
        FocalStat::Sum,
        FocalStat::Range,
        FocalStat::Median,
        FocalStat::StdDev,
    ] {
        assert_close(&algebra::focal(&band, 1, stat).unwrap(), &ref_focal(&band, 1, stat));
    }
}

#[test]
fn focal_skips_nodata_like_reference() {
    // Put a nodata hole in the middle and confirm both implementations fill it
    // from neighbours rather than treating it as zero.
    let mut band = band_from_fn(5, 5, |x, y| (x + y) as f64);
    band.grid_mut().set(2, 2, f64::NAN).unwrap();
    let expected = ref_focal(&band, 1, FocalStat::Mean);
    let got = algebra::focal(&band, 1, FocalStat::Mean).unwrap();
    assert_close(&got, &expected);
    // The hole is filled (not nodata), and equals the mean of its 8 neighbours.
    let sum: f64 = [(1, 1), (2, 1), (3, 1), (1, 2), (3, 2), (1, 3), (2, 3), (3, 3)]
        .iter()
        .map(|&(x, y)| band.get(x, y).unwrap())
        .sum();
    assert!((got.get(2, 2).unwrap() - sum / 8.0).abs() < 1e-9);
}

#[test]
fn average_downsampling_matches_block_mean_reference() {
    use tpt_gis_raster::resample;
    let band = band_from_fn(8, 8, |x, y| (x * 2 + y * 5) as f64);
    let factor = 2;
    let overview =
        resample::downsample(&band, factor, tpt_gis_raster::Resampling::Average).unwrap();

    // Independent block-mean reference.
    let (ow, oh) = (band.width() / factor, band.height() / factor);
    let mut cells = Vec::with_capacity(ow * oh);
    for oy in 0..oh {
        for ox in 0..ow {
            let mut sum = 0.0;
            let mut count = 0usize;
            for wy in oy * factor..(oy + 1) * factor {
                for wx in ox * factor..(ox + 1) * factor {
                    if let Some(v) = band.value_at(wx, wy) {
                        sum += v;
                        count += 1;
                    }
                }
            }
            cells.push(sum / count as f64);
        }
    }
    let expected =
        Band::new(Grid::new(ow, oh, cells).unwrap(), overview.transform()).with_nodata(f64::NAN);
    assert_close(&overview, &expected);
}
