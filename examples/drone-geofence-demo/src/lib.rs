//! MVP 2: Real-Time Drone Geofence & Navigation Engine.
//!
//! Demonstrates `tpt-gis-core` and `tpt-gis-geom` running the core geofencing
//! logic a flight controller needs: given a pre-loaded no-fly-zone polygon and a
//! stream of GPS fixes, detect a breach and compute the bearing/distance to the
//! nearest boundary point (the "escape vector").
//!
//! The check itself ([`check_position`]) only touches `no_std`, zero-allocation
//! APIs from `tpt-gis-core`/`tpt-gis-geom`. This crate builds `no_std` itself with
//! `--no-default-features` (see CI's `no_std` job, which cross-compiles this lib for
//! `thumbv7em-none-eabihf` and `riscv32imc-unknown-none-elf` — the latter being the
//! ESP32-C3's target) so [`check_position`] is ready to drop into real firmware
//! (e.g. an `esp-hal`-based ESP32/C3/S3 project) without changes. The `std` feature
//! is on by default so this crate's own demo binary (which uses `println!`) builds
//! normally; it is not needed by [`check_position`] itself.

#![cfg_attr(not(feature = "std"), no_std)]

use tpt_gis_core::geodesic;
use tpt_gis_core::projection::local_tangent_plane::LocalTangentPlane;
use tpt_gis_core::{Ellipsoid, GeoPoint, PlanarPoint};
use tpt_gis_geom::{Point, Polygon};

/// A small no-fly zone (a rectangle roughly 1.7km x 2.2km) over part of San Francisco,
/// as `const` data: zero heap allocation, so this is usable directly on embedded
/// targets. Vertices are `(longitude, latitude)` in decimal degrees, matching the
/// GeoJSON coordinate order convention.
pub const NO_FLY_ZONE: [Point; 5] = [
    Point::new(-122.42, 37.77),
    Point::new(-122.40, 37.77),
    Point::new(-122.40, 37.79),
    Point::new(-122.42, 37.79),
    Point::new(-122.42, 37.77),
];

/// Result of checking a single GPS fix against the no-fly zone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeofenceCheck {
    /// Whether the position is inside the no-fly zone.
    pub breached: bool,
    /// If breached, the bearing (clockwise from north, in decimal degrees) toward the
    /// nearest point on the zone's boundary — the heading to escape.
    pub escape_bearing_deg: Option<f64>,
    /// If breached, the geodesic distance to that nearest boundary point, in meters.
    pub escape_distance_m: Option<f64>,
}

/// Checks `position` against [`NO_FLY_ZONE`], returning whether it's breached and,
/// if so, the escape vector (bearing and distance to the nearest boundary point).
///
/// Longitude and latitude degrees are *not* equal in ground distance (a degree of
/// longitude shrinks by `cos(latitude)`), so comparing them directly as if they were
/// planar coordinates can pick the wrong "nearest" edge. To avoid that, this
/// projects the zone into a local East-North-Up tangent plane centered on `position`
/// (see [`LocalTangentPlane`]) before running `tpt-gis-geom`'s point-in-polygon and
/// nearest-boundary-point search in proper meters, then converts the result back to
/// geographic coordinates and computes the final bearing/distance with
/// `tpt-gis-core`'s ellipsoidal geodesic solver.
#[must_use]
pub fn check_position(position: GeoPoint) -> GeofenceCheck {
    let plane = LocalTangentPlane::new(position);

    let mut local_zone = [Point::new(0.0, 0.0); NO_FLY_ZONE.len()];
    for (vertex, local_vertex) in NO_FLY_ZONE.iter().zip(local_zone.iter_mut()) {
        let vertex_geo = GeoPoint::new(vertex.y, vertex.x);
        let local = plane.to_local(vertex_geo);
        *local_vertex = Point::new(local.x, local.y);
    }
    let zone = Polygon::from_exterior(&local_zone);

    // `position` is the tangent plane's own origin, so it always projects to (0, 0).
    let local_origin = Point::new(0.0, 0.0);

    if !zone.contains_point(local_origin) {
        return GeofenceCheck {
            breached: false,
            escape_bearing_deg: None,
            escape_distance_m: None,
        };
    }

    // The zone is a non-empty polygon, so a nearest boundary point always exists.
    let nearest_local = zone
        .nearest_boundary_point(local_origin)
        .expect("NO_FLY_ZONE has a non-empty exterior ring");
    let nearest_geo = plane.to_geographic(PlanarPoint::new(nearest_local.x, nearest_local.y));

    let result = geodesic::inverse(&Ellipsoid::WGS84, position, nearest_geo)
        .expect("a point and the nearest point on its own containing polygon are never antipodal");

    GeofenceCheck {
        breached: true,
        escape_bearing_deg: Some(result.initial_bearing_deg),
        escape_distance_m: Some(result.distance_m),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_outside_zone_is_not_breached() {
        let check = check_position(GeoPoint::new(37.760, -122.430));
        assert!(!check.breached);
        assert_eq!(check.escape_bearing_deg, None);
    }

    #[test]
    fn position_inside_zone_is_breached_with_escape_vector() {
        let check = check_position(GeoPoint::new(37.775, -122.415));
        assert!(check.breached);
        assert!(check.escape_bearing_deg.is_some());
        assert!(check.escape_distance_m.unwrap() > 0.0);
    }

    /// Regression test: at this position, the west edge (~440m away) and south edge
    /// (~555m away) are both close, but a naive degree-space (rather than meter-space)
    /// nearest-edge search picks the *south* edge — because 0.005 deg of longitude and
    /// 0.005 deg of latitude look equally "close" without accounting for
    /// `cos(latitude)` shrinking longitude's ground distance. This pins the correct,
    /// meter-accurate answer (west, ~440m) so that regresses loudly if reintroduced.
    #[test]
    fn nearest_edge_selection_accounts_for_longitude_shrinkage() {
        let check = check_position(GeoPoint::new(37.775, -122.415));
        assert!(check.breached);

        let bearing = check.escape_bearing_deg.unwrap();
        let distance = check.escape_distance_m.unwrap();
        assert!((bearing - 270.0).abs() < 1.0, "escape bearing = {bearing}");
        assert!((distance - 440.5).abs() < 1.0, "escape distance = {distance}");
    }

    #[test]
    fn escape_vector_points_toward_nearest_edge() {
        // This point is close to the west edge (lon -122.42) and far from the others,
        // so the escape bearing should point roughly west (270 deg).
        let check = check_position(GeoPoint::new(37.78, -122.419));
        assert!(check.breached);
        let bearing = check.escape_bearing_deg.unwrap();
        assert!((bearing - 270.0).abs() < 5.0, "escape bearing = {bearing}");
    }
}
