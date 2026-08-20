//! `tpt-gis-core` example: geodesic math and coordinate projections.
//!
//! Run with: `cargo run --example geodesic_demo -p tpt-gis-core`

use tpt_gis_core::datum::{geocentric_to_geodetic, geodetic_to_geocentric, Geocentric};
use tpt_gis_core::geodesic;
use tpt_gis_core::projection::local_tangent_plane::LocalTangentPlane;
use tpt_gis_core::projection::utm::{self, Zone};
use tpt_gis_core::projection::web_mercator;
use tpt_gis_core::{Ellipsoid, GeoPoint};

fn main() {
    let melbourne = GeoPoint::new(-37.8136, 144.9631);
    let sydney = GeoPoint::new(-33.8688, 151.2093);

    // Geodesic distance + bearing on the WGS84 ellipsoid (Vincenty's inverse).
    let result = geodesic::inverse(&Ellipsoid::WGS84, melbourne, sydney).unwrap();
    println!(
        "Melbourne -> Sydney: {:.1} m, initial bearing {:.1} deg",
        result.distance_m, result.initial_bearing_deg
    );

    // Web Mercator (EPSG:3857) projection, in meters.
    let xy = web_mercator::forward(melbourne);
    println!("Web Mercator: x={:.1} m, y={:.1} m", xy.x, xy.y);

    // UTM zone + projection.
    let zone = Zone::containing(melbourne);
    let utm = utm::forward(&Ellipsoid::WGS84, melbourne, zone);
    println!(
        "UTM zone {}N: easting={:.1} m, northing={:.1} m (EPSG {})",
        zone.number,
        utm.x,
        utm.y,
        zone.epsg_code()
    );

    // Local tangent plane: meters from an origin (accounts for longitude shrink).
    let plane = LocalTangentPlane::new(melbourne);
    let local = plane.to_local(sydney);
    println!("Sydney is {:.0} m east, {:.0} m north of Melbourne (local ENU)", local.x, local.y);

    // Geodetic <-> ECEF round trip.
    let gc: Geocentric = geodetic_to_geocentric(&Ellipsoid::WGS84, melbourne, 0.0);
    let (back, height) = geocentric_to_geodetic(&Ellipsoid::WGS84, gc);
    println!(
        "ECEF = ({:.0}, {:.0}, {:.0}) m; round-trip height = {:.6} m",
        gc.x, gc.y, gc.z, height
    );
    assert!((back.lat_deg - melbourne.lat_deg).abs() < 1e-9);
}
