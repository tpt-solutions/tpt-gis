//! Local tangent-plane (East-North-Up / ENU) approximation: a flat, meter-scale
//! projection centered on an origin point, valid for small areas (a few kilometers
//! across an origin's latitude).
//!
//! This exists because longitude and latitude degrees are *not* equal in ground
//! distance — one degree of longitude shrinks by a factor of `cos(latitude)` away
//! from the equator, while one degree of latitude stays roughly constant. Comparing
//! raw longitude/latitude differences as if they were Cartesian (as some small-area
//! applications are tempted to do) silently picks the wrong "nearest" point whenever
//! east-west and north-south offsets are of a similar degree-magnitude but different
//! true distances. Projecting into local ENU meters first, then doing planar
//! geometry (e.g. `tpt-gis-geom`'s nearest-boundary-point search) in that space,
//! avoids that class of bug.

use libm::{cos, pow, sin, sqrt};

use crate::float_ext::Fabs as _;
use crate::{GeoPoint, PlanarPoint};

/// WGS84 semi-major axis, in meters.
const WGS84_A: f64 = 6_378_137.0;
/// WGS84 first eccentricity squared.
const WGS84_E2: f64 = 0.006_694_379_990_14;

/// Below this `cos(latitude)` the east axis is degenerate (the origin sits on a pole).
const POLE_COS_EPSILON: f64 = 1e-12;

/// Wraps a longitude difference in degrees into `[-180, 180)`, so a plane centered
/// across the antimeridian measures the short way round.
fn wrap_delta_lon_deg(delta: f64) -> f64 {
    ((((delta + 180.0) % 360.0) + 360.0) % 360.0) - 180.0
}

/// A local East-North-Up tangent plane centered on an origin point.
///
/// Uses WGS84 radii of curvature evaluated at the origin latitude: the meridional
/// radius `M` for north and the prime-vertical radius `N` for east. This is the
/// standard flat-earth approximation for small areas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalTangentPlane {
    origin: GeoPoint,
    /// Meters per radian of longitude at the origin: `N · cos φ`.
    east_scale: f64,
    /// Meters per radian of latitude at the origin: `M`.
    north_scale: f64,
}

impl LocalTangentPlane {
    /// Constructs a tangent plane centered on `origin`.
    ///
    /// Near the poles the east axis collapses (`cos φ → 0`); on a pole itself all
    /// longitudes map to x = 0 and [`Self::to_geographic`] returns the origin longitude.
    #[must_use]
    pub fn new(origin: GeoPoint) -> Self {
        let phi = origin.lat_rad();
        let sin2 = sin(phi) * sin(phi);
        let w = sqrt(1.0 - WGS84_E2 * sin2);
        let n = WGS84_A / w;
        let m = WGS84_A * (1.0 - WGS84_E2) / pow(w, 3.0);
        let cos_phi = cos(phi);
        let east_scale = if cos_phi.fabs() < POLE_COS_EPSILON { 0.0 } else { n * cos_phi };
        Self { origin, east_scale, north_scale: m }
    }

    /// Projects `point` to local East/North meters relative to the origin.
    ///
    /// Longitude differences wrap across the antimeridian. Accurate to well under 1%
    /// of the offset distance within a few kilometers of the origin; error grows with
    /// distance from the origin latitude.
    #[must_use]
    pub fn to_local(&self, point: GeoPoint) -> PlanarPoint {
        let d_lon = wrap_delta_lon_deg(point.lon_deg - self.origin.lon_deg);
        let east = d_lon.to_radians() * self.east_scale;
        let north = (point.lat_deg - self.origin.lat_deg).to_radians() * self.north_scale;
        PlanarPoint::new(east, north)
    }

    /// The inverse of [`Self::to_local`]: recovers a geographic coordinate from
    /// local East/North meters relative to the origin.
    ///
    /// The returned longitude is normalised to `[-180, 180)`.
    #[must_use]
    pub fn to_geographic(&self, point: PlanarPoint) -> GeoPoint {
        let lon_deg = if self.east_scale.fabs() < POLE_COS_EPSILON {
            self.origin.lon_deg
        } else {
            wrap_delta_lon_deg(self.origin.lon_deg + (point.x / self.east_scale).to_degrees())
        };
        let lat_deg = self.origin.lat_deg + (point.y / self.north_scale).to_degrees();
        GeoPoint::new(lat_deg, lon_deg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_maps_to_zero() {
        let origin = GeoPoint::new(37.775, -122.415);
        let plane = LocalTangentPlane::new(origin);
        let local = plane.to_local(origin);
        assert!(local.x.fabs() < 1e-9);
        assert!(local.y.fabs() < 1e-9);
    }

    #[test]
    fn one_degree_latitude_matches_wgs84_meridional_radius() {
        // At the equator the WGS84 meridional radius is a(1 - e²), giving
        // 110 574.39 m per degree of latitude (not the spherical 111 319 m).
        let origin = GeoPoint::new(0.0, 0.0);
        let plane = LocalTangentPlane::new(origin);
        let local = plane.to_local(GeoPoint::new(1.0, 0.0));
        assert!((local.y - 110_574.39).fabs() < 0.5, "north = {}", local.y);
        assert!(local.x.fabs() < 1e-6);
    }

    /// A plane centred at 179.9°E must place -179.9° (the same meridian, 0.2° further
    /// east) a short distance east of the origin, not about 360° away.
    #[test]
    fn plane_wraps_across_the_antimeridian() {
        let plane = LocalTangentPlane::new(GeoPoint::new(0.0, 179.9));
        let east_of_origin = plane.to_local(GeoPoint::new(0.0, -179.9));
        // 0.2° of longitude at the equator is about 22.26 km, eastward.
        assert!((east_of_origin.x - 22_263.9).fabs() < 1.0, "{east_of_origin:?}");
        let back = plane.to_geographic(east_of_origin);
        assert!((back.lon_deg - (-179.9)).fabs() < 1e-9, "lon {}", back.lon_deg);
    }

    /// On a pole the east axis is degenerate; the inverse must stay finite.
    #[test]
    fn pole_origin_inverse_is_finite() {
        let plane = LocalTangentPlane::new(GeoPoint::new(90.0, 0.0));
        let g = plane.to_geographic(PlanarPoint::new(1_000.0, 500.0));
        assert!(g.lat_deg.is_finite() && g.lon_deg.is_finite());
        assert_eq!(g.lon_deg, 0.0);
    }

    #[test]
    fn longitude_degree_shrinks_away_from_equator() {
        // At 60 deg latitude, cos(60 deg) = 0.5, so a degree of longitude should
        // cover about half the ground distance it does at the equator.
        let equator = LocalTangentPlane::new(GeoPoint::new(0.0, 0.0));
        let sixty_deg = LocalTangentPlane::new(GeoPoint::new(60.0, 0.0));

        let east_at_equator = equator.to_local(GeoPoint::new(0.0, 1.0)).x;
        let east_at_60 = sixty_deg.to_local(GeoPoint::new(60.0, 1.0)).x;

        // The ellipsoidal prime-vertical radius adds a ~0.13% correction at 60°
        // (ratio ≈ 0.50126), so allow for it rather than the spherical 0.5 exactly.
        assert!((east_at_60 / east_at_equator - 0.5).fabs() < 2e-3);
    }

    #[test]
    fn forward_then_inverse_round_trips() {
        let origin = GeoPoint::new(37.775, -122.415);
        let plane = LocalTangentPlane::new(origin);
        let original = GeoPoint::new(37.780, -122.410);

        let local = plane.to_local(original);
        let recovered = plane.to_geographic(local);

        assert!((recovered.lat_deg - original.lat_deg).fabs() < 1e-9);
        assert!((recovered.lon_deg - original.lon_deg).fabs() < 1e-9);
    }
}
