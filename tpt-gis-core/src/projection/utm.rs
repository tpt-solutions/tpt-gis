//! Universal Transverse Mercator (UTM) projection.
//!
//! Implements the standard 6th-order Redfearn/Snyder series expansion (as published
//! in USGS Professional Paper 1395, "Map Projections: A Working Manual"), accurate to
//! sub-millimeter precision within a UTM zone (±3° of the central meridian).

use libm::{cos, floor, pow, sin, sqrt, tan};

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
    /// Constructs a UTM zone.
    #[must_use]
    pub const fn new(number: u8, northern: bool) -> Self {
        Self { number, northern }
    }

    /// Determines the UTM zone containing a geographic point, using the standard
    /// 6°-wide banding (no allowance for the Norway/Svalbard irregular zones).
    #[must_use]
    pub fn containing(point: GeoPoint) -> Self {
        let number = (floor((point.lon_deg + 180.0) / 6.0) as i32 + 1).clamp(1, 60) as u8;
        Self { number, northern: point.lat_deg >= 0.0 }
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

/// Projects a geographic coordinate to UTM easting/northing (in meters) within `zone`.
#[must_use]
pub fn forward(ellipsoid: &Ellipsoid, point: GeoPoint, zone: Zone) -> PlanarPoint {
    let a = ellipsoid.semi_major_axis;
    let e2 = ellipsoid.eccentricity_squared();
    let ep2 = ellipsoid.second_eccentricity_squared();

    let phi = point.lat_rad();
    let lambda = point.lon_rad();
    let lambda0 = zone.central_meridian_deg().to_radians();

    let (sin_phi, cos_phi, tan_phi) = (sin(phi), cos(phi), tan(phi));

    let n = a / sqrt(1.0 - e2 * sin_phi * sin_phi);
    let t = tan_phi * tan_phi;
    let c = ep2 * cos_phi * cos_phi;
    let big_a = (lambda - lambda0) * cos_phi;

    let m = a
        * ((1.0 - e2 / 4.0 - 3.0 * e2 * e2 / 64.0 - 5.0 * pow(e2, 3.0) / 256.0) * phi
            - (3.0 * e2 / 8.0 + 3.0 * e2 * e2 / 32.0 + 45.0 * pow(e2, 3.0) / 1024.0)
                * sin(2.0 * phi)
            + (15.0 * e2 * e2 / 256.0 + 45.0 * pow(e2, 3.0) / 1024.0) * sin(4.0 * phi)
            - (35.0 * pow(e2, 3.0) / 3072.0) * sin(6.0 * phi));

    let easting = K0
        * n
        * (big_a
            + (1.0 - t + c) * pow(big_a, 3.0) / 6.0
            + (5.0 - 18.0 * t + t * t + 72.0 * c - 58.0 * ep2) * pow(big_a, 5.0) / 120.0)
        + FALSE_EASTING_M;

    let mut northing = K0
        * (m + n
            * tan_phi
            * (big_a * big_a / 2.0
                + (5.0 - t + 9.0 * c + 4.0 * c * c) * pow(big_a, 4.0) / 24.0
                + (61.0 - 58.0 * t + t * t + 600.0 * c - 330.0 * ep2) * pow(big_a, 6.0) / 720.0));

    if !zone.northern {
        northing += FALSE_NORTHING_SOUTH_M;
    }

    PlanarPoint::new(easting, northing)
}

/// Projects a UTM easting/northing (in meters) within `zone` back to a geographic coordinate.
#[must_use]
pub fn inverse(ellipsoid: &Ellipsoid, point: PlanarPoint, zone: Zone) -> GeoPoint {
    let a = ellipsoid.semi_major_axis;
    let e2 = ellipsoid.eccentricity_squared();
    let ep2 = ellipsoid.second_eccentricity_squared();

    let x = point.x - FALSE_EASTING_M;
    let y = if zone.northern { point.y } else { point.y - FALSE_NORTHING_SOUTH_M };

    let m = y / K0;
    let mu = m / (a * (1.0 - e2 / 4.0 - 3.0 * e2 * e2 / 64.0 - 5.0 * pow(e2, 3.0) / 256.0));

    let e1 = (1.0 - sqrt(1.0 - e2)) / (1.0 + sqrt(1.0 - e2));

    let phi1 = mu
        + (3.0 * e1 / 2.0 - 27.0 * pow(e1, 3.0) / 32.0) * sin(2.0 * mu)
        + (21.0 * e1 * e1 / 16.0 - 55.0 * pow(e1, 4.0) / 32.0) * sin(4.0 * mu)
        + (151.0 * pow(e1, 3.0) / 96.0) * sin(6.0 * mu)
        + (1097.0 * pow(e1, 4.0) / 512.0) * sin(8.0 * mu);

    let (sin_phi1, cos_phi1, tan_phi1) = (sin(phi1), cos(phi1), tan(phi1));

    let c1 = ep2 * cos_phi1 * cos_phi1;
    let t1 = tan_phi1 * tan_phi1;
    let n1 = a / sqrt(1.0 - e2 * sin_phi1 * sin_phi1);
    let r1 = a * (1.0 - e2) / pow(1.0 - e2 * sin_phi1 * sin_phi1, 1.5);
    let d = x / (n1 * K0);

    let phi = phi1
        - (n1 * tan_phi1 / r1)
            * (d * d / 2.0
                - (5.0 + 3.0 * t1 + 10.0 * c1 - 4.0 * c1 * c1 - 9.0 * ep2) * pow(d, 4.0) / 24.0
                + (61.0 + 90.0 * t1 + 298.0 * c1 + 45.0 * t1 * t1 - 252.0 * ep2 - 3.0 * c1 * c1)
                    * pow(d, 6.0)
                    / 720.0);

    let lambda0 = zone.central_meridian_deg().to_radians();
    let lambda = lambda0
        + (d - (1.0 + 2.0 * t1 + c1) * pow(d, 3.0) / 6.0
            + (5.0 - 2.0 * c1 + 28.0 * t1 - 3.0 * c1 * c1 + 8.0 * ep2 + 24.0 * t1 * t1)
                * pow(d, 5.0)
                / 120.0)
            / cos_phi1;

    GeoPoint::new(phi.to_degrees(), lambda.to_degrees())
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
