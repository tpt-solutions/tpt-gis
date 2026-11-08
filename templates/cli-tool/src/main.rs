//! Prints the ellipsoidal (WGS84) distance and initial bearing between two points.
//!
//! Usage: `{{project-name}} LAT1 LON1 LAT2 LON2`

use tpt_gis::core::geodesic;
use tpt_gis::core::{Ellipsoid, GeoPoint};

fn parse_degrees(arg: Option<String>, name: &str) -> Result<f64, String> {
    let text = arg.ok_or_else(|| format!("missing {name}"))?;
    text.parse::<f64>().map_err(|_| format!("{name} must be a number, got '{text}'"))
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let lat1 = parse_degrees(args.next(), "LAT1")?;
    let lon1 = parse_degrees(args.next(), "LON1")?;
    let lat2 = parse_degrees(args.next(), "LAT2")?;
    let lon2 = parse_degrees(args.next(), "LON2")?;

    let a = GeoPoint::new(lat1, lon1);
    let b = GeoPoint::new(lat2, lon2);
    let result = geodesic::inverse(&Ellipsoid::WGS84, a, b).map_err(|e| format!("{e:?}"))?;
    println!("distance: {:.3} m", result.distance_m);
    println!("initial bearing: {:.3} degrees", result.initial_bearing_deg);
    Ok(())
}

fn main() {
    if let Err(message) = run() {
        eprintln!("error: {message}");
        std::process::exit(2);
    }
}
