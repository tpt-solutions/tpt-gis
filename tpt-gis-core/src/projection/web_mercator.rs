//! Web Mercator (EPSG:3857), the spherical projection used by nearly all web map tiles
//! (Google Maps, OpenStreetMap, Mapbox, etc).
//!
//! Web Mercator projects WGS84 longitude/latitude as if the Earth were a sphere of
//! radius equal to the WGS84 semi-major axis — it does *not* account for the
//! ellipsoid's flattening. This is a deliberate simplification inherited from its
//! origin as a fast web-tiling projection, not a geodetic error.

use libm::{atan, exp, log, tan};

use crate::{GeoPoint, PlanarPoint};

/// Radius used by the Web Mercator sphere (the WGS84 semi-major axis).
pub const EARTH_RADIUS_M: f64 = 6_378_137.0;

/// Maximum latitude representable in Web Mercator (beyond this, `y` diverges to infinity).
pub const MAX_LATITUDE_DEG: f64 = 85.051_128_78;

/// Projects a geographic coordinate to Web Mercator meters.
///
/// Latitudes outside ±[`MAX_LATITUDE_DEG`] are clamped, matching the behavior of
/// standard web mapping libraries.
#[must_use]
pub fn forward(point: GeoPoint) -> PlanarPoint {
    let lat_deg = point.lat_deg.clamp(-MAX_LATITUDE_DEG, MAX_LATITUDE_DEG);
    let lat_rad = lat_deg.to_radians();

    // Longitudes outside [-180, 180) wrap, so 190°E projects like -170°.
    let lon_deg = ((((point.lon_deg + 180.0) % 360.0) + 360.0) % 360.0) - 180.0;
    let x = EARTH_RADIUS_M * lon_deg.to_radians();
    let y = EARTH_RADIUS_M * log(tan(core::f64::consts::FRAC_PI_4 + lat_rad / 2.0));

    PlanarPoint::new(x, y)
}

/// Projects a Web Mercator coordinate (in meters) back to geographic longitude/latitude.
#[must_use]
pub fn inverse(point: PlanarPoint) -> GeoPoint {
    let lon_rad = point.x / EARTH_RADIUS_M;
    let lat_rad = 2.0 * atan(exp(point.y / EARTH_RADIUS_M)) - core::f64::consts::FRAC_PI_2;

    GeoPoint::new(lat_rad.to_degrees(), lon_rad.to_degrees())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Longitudes beyond the antimeridian wrap to the equivalent point.
    #[test]
    fn longitude_wraps_at_the_antimeridian() {
        let east = forward(GeoPoint::new(10.0, 190.0));
        let west = forward(GeoPoint::new(10.0, -170.0));
        assert!((east.x - west.x).abs() < 1e-6, "{} vs {}", east.x, west.x);
        assert!((east.y - west.y).abs() < 1e-9);
    }

    #[test]
    fn origin_maps_to_origin() {
        let p = forward(GeoPoint::new(0.0, 0.0));
        assert!(p.x.abs() < 1e-9);
        assert!(p.y.abs() < 1e-9);
    }

    #[test]
    fn forward_then_inverse_round_trips() {
        let original = GeoPoint::new(51.5074, -0.1278); // London
        let projected = forward(original);
        let recovered = inverse(projected);

        assert!((recovered.lat_deg - original.lat_deg).abs() < 1e-9);
        assert!((recovered.lon_deg - original.lon_deg).abs() < 1e-9);
    }

    #[test]
    fn forward_matches_known_reference() {
        let point = GeoPoint::new(40.7484, -73.9857);
        let projected = forward(point);
        assert!((projected.x - (-8236050.45)).abs() < 1.0, "x = {}", projected.x);
        assert!((projected.y - 4975301.25).abs() < 1.0, "y = {}", projected.y);
    }
}
