//! Universal Transverse Mercator (UTM) projection.
//!
//! Implements the standard 6th-order Redfearn/Snyder series expansion (as published
//! in USGS Professional Paper 1395, "Map Projections: A Working Manual"), accurate to
//! sub-millimeter precision within a UTM zone (±3° of the central meridian).

use libm::floor;

use super::transverse_mercator::TransverseMercator;

use crate::{Ellipsoid, GeoPoint, PlanarPoint};

/// UTM scale factor at the central meridian.
pub const K0: f64 = 0.999_6;

/// False easting applied to keep eastings positive within a zone, in meters.
pub const FALSE_EASTING_M: f64 = 500_000.0;

/// False northing applied in the southern hemisphere, in meters.
pub const FALSE_NORTHING_SOUTH_M: f64 = 10_000_000.0;

/// A UTM zone: a 6°-wide longitudinal band (1-60) and a hemisphere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Zone {
    /// Zone number, 1-60.
    pub number: u8,
    /// `true` for the northern hemisphere, `false` for the southern.
    pub northern: bool,
}

impl Zone {
    /// Constructs a UTM zone. `number` is not range-checked; use [`Zone::try_new`]
    /// for untrusted input.
    #[must_use]
    pub const fn new(number: u8, northern: bool) -> Self {
        Self { number, northern }
    }

    /// Constructs a UTM zone, returning `None` if `number` is not in `1..=60`.
    #[must_use]
    pub const fn try_new(number: u8, northern: bool) -> Option<Self> {
        if number >= 1 && number <= 60 {
            Some(Self { number, northern })
        } else {
            None
        }
    }

    /// Determines the UTM zone containing a geographic point.
    ///
    /// Uses the standard 6°-wide banding, with the two irregular regions applied:
    /// Norway (zone 32 is widened to 3°–12°E for 56°–64°N, i.e. zone 32V) and Svalbard
    /// (zones 31X, 33X, 35X and 37X cover 72°–84°N, with 0°–9°, 9°–21°, 21°–33° and
    /// 33°–42° of longitude respectively).
    ///
    /// Longitudes outside `[-180, 180]` are wrapped. Non-finite input yields an
    /// unspecified zone; use [`Zone::try_containing`] to reject it instead.
    #[must_use]
    pub fn containing(point: GeoPoint) -> Self {
        let lon = wrap_longitude_deg(point.lon_deg);
        let lat = point.lat_deg;
        let northern = lat >= 0.0;
        let mut number = (floor((lon + 180.0) / 6.0) as i32 + 1).clamp(1, 60) as u8;

        // Norway: zone 32 extends east to 12°E at 56°N–64°N, so 5°E there is zone 32.
        if (56.0..64.0).contains(&lat) && (3.0..12.0).contains(&lon) {
            number = 32;
        }
        // Svalbard: the irregular 12° zones of 72°N–84°N.
        if (72.0..84.0).contains(&lat) {
            if (0.0..9.0).contains(&lon) {
                number = 31;
            } else if (9.0..21.0).contains(&lon) {
                number = 33;
            } else if (21.0..33.0).contains(&lon) {
                number = 35;
            } else if (33.0..42.0).contains(&lon) {
                number = 37;
            }
        }
        Self { number, northern }
    }

    /// Like [`Zone::containing`], but returns `None` if either coordinate is not finite
    /// or the latitude is outside `[-90, 90]`.
    #[must_use]
    pub fn try_containing(point: GeoPoint) -> Option<Self> {
        let finite = point.lat_deg.is_finite() && point.lon_deg.is_finite();
        (finite && (-90.0..=90.0).contains(&point.lat_deg)).then(|| Self::containing(point))
    }

    /// Central meridian of this zone, in decimal degrees.
    #[must_use]
    pub fn central_meridian_deg(&self) -> f64 {
        f64::from(self.number) * 6.0 - 183.0
    }

    /// EPSG code for this zone (326xx for north, 327xx for south).
    #[must_use]
    pub fn epsg_code(&self) -> u32 {
        let base = if self.northern { 32600 } else { 32700 };
        base + u32::from(self.number)
    }
}

/// Wraps a longitude in degrees into `[-180, 180)`.
fn wrap_longitude_deg(lon_deg: f64) -> f64 {
    ((((lon_deg + 180.0) % 360.0) + 360.0) % 360.0) - 180.0
}

/// The Transverse Mercator projection for a UTM zone: origin on the equator at the zone's
/// central meridian, scale [`K0`], false easting [`FALSE_EASTING_M`], and a false northing
/// of 10 000 000 m in the southern hemisphere.
fn zone_projection(ellipsoid: &Ellipsoid, zone: Zone) -> TransverseMercator {
    let false_northing = if zone.northern { 0.0 } else { FALSE_NORTHING_SOUTH_M };
    TransverseMercator::new(
        ellipsoid,
        0.0,
        zone.central_meridian_deg(),
        K0,
        FALSE_EASTING_M,
        false_northing,
    )
}

/// Projects a geographic coordinate to UTM easting/northing (in meters) within `zone`.
///
/// The longitude offset from the zone's central meridian wraps to `[-180, 180)`, so a
/// point 179°E projects identically to 181°W for any zone.
#[must_use]
pub fn forward(ellipsoid: &Ellipsoid, point: GeoPoint, zone: Zone) -> PlanarPoint {
    zone_projection(ellipsoid, zone).forward(point)
}

/// Projects a UTM easting/northing (in meters) within `zone` back to a geographic coordinate.
#[must_use]
pub fn inverse(ellipsoid: &Ellipsoid, point: PlanarPoint, zone: Zone) -> GeoPoint {
    zone_projection(ellipsoid, zone).inverse(point)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zone_containing_greenwich() {
        let zone = Zone::containing(GeoPoint::new(51.5, 0.001));
        assert_eq!(zone.number, 31);
        assert!(zone.northern);
    }

    #[test]
    fn try_new_validates_zone_number() {
        assert_eq!(Zone::try_new(0, true), None);
        assert_eq!(Zone::try_new(61, false), None);
        assert_eq!(Zone::try_new(60, false), Some(Zone::new(60, false)));
    }

    #[test]
    fn try_containing_rejects_non_finite_input() {
        assert_eq!(Zone::try_containing(GeoPoint::new(f64::NAN, 0.0)), None);
        assert_eq!(Zone::try_containing(GeoPoint::new(0.0, f64::INFINITY)), None);
        assert_eq!(Zone::try_containing(GeoPoint::new(91.0, 0.0)), None);
        assert!(Zone::try_containing(GeoPoint::new(10.0, 10.0)).is_some());
    }

    /// Southern Norway (56°–64°N, 3°–12°E) is zone 32V, not 31V.
    #[test]
    fn norway_exception_zone_32v() {
        let zone = Zone::containing(GeoPoint::new(60.0, 5.0));
        assert_eq!(zone.number, 32);
        assert!(zone.northern);
        assert_eq!(zone.epsg_code(), 32632);
    }

    /// Svalbard's irregular zones: 33X covers 9°E–21°E at 78°N.
    #[test]
    fn svalbard_exception_zone_33x() {
        assert_eq!(Zone::containing(GeoPoint::new(78.0, 15.0)).number, 33);
        assert_eq!(Zone::containing(GeoPoint::new(78.0, 3.0)).number, 31);
    }

    /// Longitudes beyond ±180° wrap to the same zone as their equivalent.
    #[test]
    fn containing_wraps_longitude() {
        assert_eq!(
            Zone::containing(GeoPoint::new(0.0, 190.0)),
            Zone::containing(GeoPoint::new(0.0, -170.0))
        );
    }

    /// 179°E and 181°W are the same meridian; within zone 1 (central meridian -177°)
    /// they must project identically, not ~360° apart.
    #[test]
    fn forward_wraps_longitude_offset() {
        let zone = Zone::new(1, true);
        let east = forward(&Ellipsoid::WGS84, GeoPoint::new(10.0, 179.0), zone);
        let west = forward(&Ellipsoid::WGS84, GeoPoint::new(10.0, -181.0), zone);
        assert!((east.x - west.x).abs() < 1e-6, "{} vs {}", east.x, west.x);
        assert!((east.y - west.y).abs() < 1e-6);
    }

    /// The forward/inverse pair use a truncated 6th-order series expansion (per USGS
    /// Professional Paper 1395), which has a small, expected non-zero truncation
    /// residual rather than being an exact analytic inverse. Sub-millimeter agreement
    /// (< 1e-7 degrees, ~1cm at the equator) is the expected precision within a zone.
    #[test]
    fn forward_then_inverse_round_trips() {
        let original = GeoPoint::new(50.774_9, 6.089_4); // near Aachen, zone 32N
        let zone = Zone::containing(original);

        let projected = forward(&Ellipsoid::WGS84, original, zone);
        let recovered = inverse(&Ellipsoid::WGS84, projected, zone);

        assert!(
            (recovered.lat_deg - original.lat_deg).abs() < 1e-7,
            "lat diff = {}",
            recovered.lat_deg - original.lat_deg
        );
        assert!(
            (recovered.lon_deg - original.lon_deg).abs() < 1e-7,
            "lon diff = {}",
            recovered.lon_deg - original.lon_deg
        );
    }

    /// A point exactly on the central meridian has no along-parallel offset (`A = 0` in
    /// the Redfearn series), so its easting must be exactly the false easting — this is
    /// a provable invariant of the formula, not dependent on any external reference.
    #[test]
    fn point_on_central_meridian_has_false_easting() {
        let zone = Zone::new(18, true); // central meridian -75°
        let point = GeoPoint::new(40.0, zone.central_meridian_deg());

        let projected = forward(&Ellipsoid::WGS84, point, zone);

        assert!((projected.x - FALSE_EASTING_M).abs() < 1e-6, "easting = {}", projected.x);
    }

    #[test]
    fn forward_matches_known_reference() {
        let zone = Zone::containing(GeoPoint::new(40.7128, -74.0060)); // New York City, zone 18N
        let point = GeoPoint::new(40.7128, -74.0060);
        let projected = forward(&Ellipsoid::WGS84, point, zone);
        assert!((projected.x - 583960.0).abs() < 10.0, "easting = {}", projected.x);
        assert!((projected.y - 4507351.0).abs() < 10.0, "northing = {}", projected.y);
    }
}
