//! `tpt-gis` facade example: using multiple engine crates through one dependency.
//!
//! Run with: `cargo run --example facade_demo -p tpt-gis`

use tpt_gis::core::{geodesic, Ellipsoid, GeoPoint};
use tpt_gis::geom::Point;

fn main() {
    let a = GeoPoint::new(-37.8136, 144.9631);
    let b = GeoPoint::new(-33.8688, 151.2093);

    // Geodesic distance (core crate).
    let distance_m = geodesic::inverse(&Ellipsoid::WGS84, a, b).unwrap().distance_m;
    println!("distance = {distance_m} m");

    // Planar point (geom crate).
    let p = Point::new(1.0, 2.0);
    println!("point = ({}, {})", p.x, p.y);
}
