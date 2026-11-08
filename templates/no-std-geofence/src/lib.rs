//! A geofence check that needs no allocator and no standard library, so it can run on
//! firmware. The zone is `const` data, stored in flash rather than RAM.

#![no_std]

use tpt_gis_geom::{Point, Polygon};

/// The exterior ring of the zone, as (longitude, latitude) pairs. The first and last
/// points must match. Replace these with your own zone (for example from
/// `tptgis geofence-compile`).
const ZONE: [Point; 5] = [
    Point::new(-0.10, 51.50),
    Point::new(-0.05, 51.50),
    Point::new(-0.05, 51.52),
    Point::new(-0.10, 51.52),
    Point::new(-0.10, 51.50),
];

/// Returns `true` when `(lon, lat)` lies inside the zone.
#[must_use]
pub fn is_inside_zone(lon: f64, lat: f64) -> bool {
    Polygon::from_exterior(&ZONE).contains_point(Point::new(lon, lat))
}

#[cfg(test)]
mod tests {
    use super::is_inside_zone;

    #[test]
    fn point_inside_the_zone_is_detected() {
        assert!(is_inside_zone(-0.07, 51.51));
    }

    #[test]
    fn point_outside_the_zone_is_not() {
        assert!(!is_inside_zone(0.0, 51.51));
    }
}
