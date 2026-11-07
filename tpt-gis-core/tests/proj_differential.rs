//! Differential test: compares `tpt-gis-core` projections with PROJ (via rasterio, GDAL
//! 3.12). The reference coordinates are in `tests/fixtures/proj_points.csv`: 200 UTM points
//! within 3° of their zone's central meridian (north and south), 100 Lambert-93 points over
//! France, 100 Conus Albers points over the contiguous US, and 100 Web Mercator points.

use std::path::PathBuf;

use tpt_gis_core::projection::albers::Albers;
use tpt_gis_core::projection::lcc::LambertConformal;
use tpt_gis_core::projection::utm::{self, Zone};
use tpt_gis_core::projection::web_mercator;
use tpt_gis_core::{Ellipsoid, GeoPoint, PlanarPoint};

struct Row {
    crs: String,
    arg: i32,
    lat: f64,
    lon: f64,
    x: f64,
    y: f64,
}

fn load() -> Vec<Row> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/proj_points.csv");
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split(';').collect();
            Row {
                crs: f[0].to_string(),
                arg: f[1].parse().unwrap(),
                lat: f[2].parse().unwrap(),
                lon: f[3].parse().unwrap(),
                x: f[4].parse().unwrap(),
                y: f[5].parse().unwrap(),
            }
        })
        .collect()
}

fn ours(row: &Row) -> PlanarPoint {
    let geo = GeoPoint::new(row.lat, row.lon);
    match row.crs.as_str() {
        "utm" => {
            let zone = Zone::new(row.arg.unsigned_abs() as u8, row.arg > 0);
            utm::forward(&Ellipsoid::WGS84, geo, zone)
        }
        "lambert93" => LambertConformal::lambert93().forward(geo),
        "albers_conus" => Albers::conus_albers().forward(geo),
        "web_mercator" => web_mercator::forward(geo),
        other => panic!("unknown crs {other}"),
    }
}

/// The largest error, in metres, for each CRS tag, plus the total count.
fn max_errors(rows: &[Row]) -> Vec<(String, f64, usize)> {
    let mut tags: Vec<(String, f64, usize)> = Vec::new();
    for row in rows {
        let p = ours(row);
        let err = (p.x - row.x).hypot(p.y - row.y);
        match tags.iter_mut().find(|(t, _, _)| *t == row.crs) {
            Some(entry) => {
                entry.1 = entry.1.max(err);
                entry.2 += 1;
            }
            None => tags.push((row.crs.clone(), err, 1)),
        }
    }
    tags
}

#[test]
fn projections_agree_with_proj() {
    let rows = load();
    let errors = max_errors(&rows);
    for (tag, max_m, n) in &errors {
        println!("{tag}: {n} points, max error {max_m:.9} m");
    }
    // Tolerances in metres, set from the measured maxima (printed above) with headroom:
    // UTM uses a 6th-order series against PROJ's Krüger series (max about 1 mm within 3° of
    // the central meridian); Lambert-93 and Albers agree to about 7 µm; Web Mercator to about
    // 34 µm, a difference whose cause this test does not pin down.
    let tolerance = |tag: &str| match tag {
        "utm" => 2e-3,
        "lambert93" => 1e-4,
        "albers_conus" => 1e-4,
        "web_mercator" => 1e-4,
        _ => 0.0,
    };
    for (tag, max_m, n) in &errors {
        assert!(*n > 0, "no points for {tag}");
        assert!(*max_m <= tolerance(tag), "{tag}: max error {max_m} m exceeds {}", tolerance(tag));
    }
}
