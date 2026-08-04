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

use libm::cos;

use crate::projection::web_mercator::EARTH_RADIUS_M;
use crate::{GeoPoint, PlanarPoint};

/// A local East-North-Up tangent plane centered on an origin point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalTangentPlane {
    origin: GeoPoint,
    cos_origin_lat: f64,
}

impl LocalTangentPlane {
    /// Constructs a tangent plane centered on `origin`.
    #[must_use]
    pub fn new(origin: GeoPoint) -> Self {
        Self { origin, cos_origin_lat: cos(origin.lat_rad()) }
    }

    /// Projects `point` to local East/North meters relative to the origin.
    ///
    /// Accurate to well under 1% of the offset distance within a few kilometers of
    /// the origin; error grows with distance and with proximity to the poles (where
    /// `cos(latitude)` shrinks toward zero).
    #[must_use]
    pub fn to_local(&self, point: GeoPoint) -> PlanarPoint {
        let east = (point.lon_deg - self.origin.lon_deg).to_radians()
            * EARTH_RADIUS_M
            * self.cos_origin_lat;
        let north = (point.lat_deg - self.origin.lat_deg).to_radians() * EARTH_RADIUS_M;
        PlanarPoint::new(east, north)
    }

    /// The inverse of [`Self::to_local`]: recovers a geographic coordinate from
    /// local East/North meters relative to the origin.
    #[must_use]
    pub fn to_geographic(&self, point: PlanarPoint) -> GeoPoint {
        let lon_deg =
            self.origin.lon_deg + (point.x / (EARTH_RADIUS_M * self.cos_origin_lat)).to_degrees();
        let lat_deg = self.origin.lat_deg + (point.y / EARTH_RADIUS_M).to_degrees();
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
        assert!(local.x.abs() < 1e-9);
        assert!(local.y.abs() < 1e-9);
    }

    #[test]
    fn one_degree_latitude_is_about_111_km() {
        let origin = GeoPoint::new(0.0, 0.0);
        let plane = LocalTangentPlane::new(origin);
        let local = plane.to_local(GeoPoint::new(1.0, 0.0));
        assert!((local.y - 111_319.49).abs() < 1.0, "north = {}", local.y);
        assert!(local.x.abs() < 1e-6);
    }

    #[test]
    fn longitude_degree_shrinks_away_from_equator() {
        // At 60 deg latitude, cos(60 deg) = 0.5, so a degree of longitude should
        // cover about half the ground distance it does at the equator.
        let equator = LocalTangentPlane::new(GeoPoint::new(0.0, 0.0));
        let sixty_deg = LocalTangentPlane::new(GeoPoint::new(60.0, 0.0));

        let east_at_equator = equator.to_local(GeoPoint::new(0.0, 1.0)).x;
        let east_at_60 = sixty_deg.to_local(GeoPoint::new(60.0, 1.0)).x;

        assert!((east_at_60 / east_at_equator - 0.5).abs() < 1e-3);
    }

    #[test]
    fn forward_then_inverse_round_trips() {
        let origin = GeoPoint::new(37.775, -122.415);
        let plane = LocalTangentPlane::new(origin);
        let original = GeoPoint::new(37.780, -122.410);

        let local = plane.to_local(original);
        let recovered = plane.to_geographic(local);

        assert!((recovered.lat_deg - original.lat_deg).abs() < 1e-9);
        assert!((recovered.lon_deg - original.lon_deg).abs() < 1e-9);
    }
}
