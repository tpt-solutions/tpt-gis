//! `tpt-gis-core` example: ellipsoids, the CRS/EPSG registry, and datum (Helmert)
//! transforms between reference frames.
//!
//! Run with: `cargo run --example datum_demo -p tpt-gis-core`

use tpt_gis_core::datum::{
    geocentric_to_geodetic, geodetic_to_geocentric, transform_geocentric, HelmertParams,
};
use tpt_gis_core::{Crs, Ellipsoid, GeoPoint};

fn main() {
    // --- Reference ellipsoids ------------------------------------------------
    let wgs84 = Ellipsoid::WGS84;
    let grs80 = Ellipsoid::GRS80;
    println!("WGS84 semi-minor axis : {:.3} m", wgs84.semi_minor_axis());
    println!("GRS80 semi-minor axis: {:.3} m", grs80.semi_minor_axis());
    // GRS80 is ~0.1 mm flatter — numerically near-identical to WGS84.
    let diff = (wgs84.semi_minor_axis() - grs80.semi_minor_axis()).abs();
    println!("semi-minor axis difference: {diff:.6} m");

    // --- CRS registry / EPSG lookup ------------------------------------------
    let web = Crs::WebMercator;
    println!("{web:?} -> EPSG {}", web.epsg_code());
    if let Some(crs) = tpt_gis_core::epsg::lookup(32618) {
        println!("EPSG 32618 resolves to {crs:?}");
    }
    if tpt_gis_core::epsg::lookup(27700).is_none() {
        println!("EPSG 27700 (British National Grid) is not yet in the registry");
    }

    // --- Datum transform via a Helmert 7-parameter (Bursa-Wolf) transform ----
    // A synthetic 10 ppm scale + (1, 2, 3) m translation, so the shift is visible.
    let params = HelmertParams::from_arcsec_ppm(1.0, 2.0, 3.0, 0.0, 0.0, 0.0, 10.0);
    let melbourne = GeoPoint::new(-37.8136, 144.9631);

    // Old frame -> geocentric (ECEF) -> Helmert -> new frame (geodetic).
    let gc_old = geodetic_to_geocentric(&wgs84, melbourne, 0.0);
    let gc_new = transform_geocentric(&params, gc_old);
    let (moved, height) = geocentric_to_geodetic(&wgs84, gc_new);
    println!(
        "10 ppm + (1,2,3) m transform shifts Melbourne to ({:.6}, {:.6}), h = {height:.3} m",
        moved.lat_deg, moved.lon_deg
    );
    assert!((moved.lat_deg - melbourne.lat_deg).abs() < 0.01);
}
