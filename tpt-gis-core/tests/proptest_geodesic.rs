//! Property-based tests for geodesic invariants.
//!
//! Verifies symmetry, non-negativity, bearing normalization, and round-trip
//! consistency for Vincenty's formulae on the WGS84 ellipsoid.

#![allow(missing_docs)]

use proptest::prelude::*;
use tpt_gis_core::{geodesic, Ellipsoid, GeoPoint};

proptest! {
    #[test]
    fn geodesic_distance_is_non_negative(
        lat1 in -80.0..80.0,
        lon1 in -180.0..180.0,
        lat2 in -80.0..80.0,
        lon2 in -180.0..180.0,
    ) {
        let p1 = GeoPoint::new(lat1, lon1);
        let p2 = GeoPoint::new(lat2, lon2);
        if let Ok(result) = geodesic::inverse(&Ellipsoid::WGS84, p1, p2) {
            prop_assert!(result.distance_m >= 0.0, "distance is negative: {}", result.distance_m);
        }
    }

    #[test]
    fn geodesic_distance_is_symmetric(
        lat1 in -80.0..80.0,
        lon1 in -180.0..180.0,
        lat2 in -80.0..80.0,
        lon2 in -180.0..180.0,
    ) {
        let p1 = GeoPoint::new(lat1, lon1);
        let p2 = GeoPoint::new(lat2, lon2);
        let r12 = geodesic::inverse(&Ellipsoid::WGS84, p1, p2);
        let r21 = geodesic::inverse(&Ellipsoid::WGS84, p2, p1);
        if let (Ok(r12), Ok(r21)) = (r12, r21) {
            prop_assert!(
                (r12.distance_m - r21.distance_m).abs() < 1e-3,
                "distance not symmetric: {} != {}",
                r12.distance_m, r21.distance_m
            );
        }
    }

    #[test]
    fn geodesic_coincident_points_have_zero_distance(
        lat in -80.0..80.0,
        lon in -180.0..180.0,
    ) {
        let p = GeoPoint::new(lat, lon);
        let result = geodesic::inverse(&Ellipsoid::WGS84, p, p).unwrap();
        prop_assert!(result.distance_m < 1e-6, "distance to self is not zero: {}", result.distance_m);
    }

    #[test]
    fn geodesic_bearings_are_normalized(
        lat1 in -80.0..80.0,
        lon1 in -180.0..180.0,
        lat2 in -80.0..80.0,
        lon2 in -180.0..180.0,
    ) {
        let p1 = GeoPoint::new(lat1, lon1);
        let p2 = GeoPoint::new(lat2, lon2);
        if let Ok(result) = geodesic::inverse(&Ellipsoid::WGS84, p1, p2) {
            prop_assert!(result.initial_bearing_deg >= 0.0);
            prop_assert!(result.initial_bearing_deg < 360.0);
            prop_assert!(result.final_bearing_deg >= 0.0);
            prop_assert!(result.final_bearing_deg < 360.0);
        }
    }

    #[test]
    fn geodesic_direct_round_trip(
        lat1 in -80.0..80.0,
        lon1 in -180.0..180.0,
        lat2 in -80.0..80.0,
        lon2 in -180.0..180.0,
    ) {
        let p1 = GeoPoint::new(lat1, lon1);
        let p2 = GeoPoint::new(lat2, lon2);
        let inverse = geodesic::inverse(&Ellipsoid::WGS84, p1, p2);
        if let Ok(inv) = inverse {
            let direct = geodesic::direct(&Ellipsoid::WGS84, p1, inv.initial_bearing_deg, inv.distance_m);
            let dest = direct.destination;
            let dlat = (dest.lat_deg - p2.lat_deg).abs();
            let mut dlon = (dest.lon_deg - p2.lon_deg).abs();
            if dlon > 180.0 {
                dlon = 360.0 - dlon;
            }
            prop_assert!(dlat < 1e-4, "latitude drift: {} deg", dlat);
            prop_assert!(dlon < 1e-4, "longitude drift: {} deg", dlon);
        }
    }
}
