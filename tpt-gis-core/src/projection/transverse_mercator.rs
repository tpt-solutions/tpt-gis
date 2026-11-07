//! Transverse Mercator projection with an arbitrary origin, scale factor and false
//! origin, on an ellipsoid.
//!
//! This is the Redfearn/Snyder series of USGS Professional Paper 1395, the same series
//! that [`crate::projection::utm`] uses. UTM is the special case with origin latitude
//! 0, scale 0.9996, and a 500 000 m false easting. Use [`TransverseMercator`] directly
//! for other national grids that are Transverse Mercator, such as Gauss-Krüger zones.
//!
//! The series is accurate to well under a millimetre within a few degrees of the central
//! meridian. It degrades beyond roughly ±10° of longitude, which is why grids are
//! usually zoned.

use libm::{cos, pow, sin, sqrt, tan};

use crate::{Ellipsoid, GeoPoint, PlanarPoint};

/// A Transverse Mercator projection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransverseMercator {
    a: f64,
    e2: f64,
    ep2: f64,
    /// Meridional arc length from the equator to the origin latitude, in meters.
    m0: f64,
    lon0_deg: f64,
    k0: f64,
    false_easting: f64,
    false_northing: f64,
}

/// The meridional arc length from the equator to `phi`, in meters.
fn meridional_arc(a: f64, e2: f64, phi: f64) -> f64 {
    a * ((1.0 - e2 / 4.0 - 3.0 * e2 * e2 / 64.0 - 5.0 * pow(e2, 3.0) / 256.0) * phi
        - (3.0 * e2 / 8.0 + 3.0 * e2 * e2 / 32.0 + 45.0 * pow(e2, 3.0) / 1024.0) * sin(2.0 * phi)
        + (15.0 * e2 * e2 / 256.0 + 45.0 * pow(e2, 3.0) / 1024.0) * sin(4.0 * phi)
        - (35.0 * pow(e2, 3.0) / 3072.0) * sin(6.0 * phi))
}

/// Wraps a longitude difference in degrees into `[-180, 180)`.
fn wrap_deg(delta: f64) -> f64 {
    ((((delta + 180.0) % 360.0) + 360.0) % 360.0) - 180.0
}

impl TransverseMercator {
    /// Builds the projection.
    ///
    /// `lat0_deg`/`lon0_deg` is the origin (the point that maps to the false origin),
    /// `k0` is the scale factor on the central meridian, and the false easting and
    /// northing are in meters.
    #[must_use]
    pub fn new(
        ellipsoid: &Ellipsoid,
        lat0_deg: f64,
        lon0_deg: f64,
        k0: f64,
        false_easting: f64,
        false_northing: f64,
    ) -> Self {
        let a = ellipsoid.semi_major_axis;
        let e2 = ellipsoid.eccentricity_squared();
        Self {
            a,
            e2,
            ep2: ellipsoid.second_eccentricity_squared(),
            m0: meridional_arc(a, e2, lat0_deg.to_radians()),
            lon0_deg,
            k0,
            false_easting,
            false_northing,
        }
    }

    /// Projects a geographic coordinate to easting/northing in meters.
    ///
    /// The longitude offset from the central meridian wraps across the antimeridian.
    #[must_use]
    pub fn forward(&self, point: GeoPoint) -> PlanarPoint {
        let phi = point.lat_rad();
        let big_a = wrap_deg(point.lon_deg - self.lon0_deg).to_radians() * cos(phi);
        let (sin_phi, cos_phi, tan_phi) = (sin(phi), cos(phi), tan(phi));

        let n = self.a / sqrt(1.0 - self.e2 * sin_phi * sin_phi);
        let t = tan_phi * tan_phi;
        let c = self.ep2 * cos_phi * cos_phi;
        let m = meridional_arc(self.a, self.e2, phi);

        let easting = self.k0
            * n
            * (big_a
                + (1.0 - t + c) * pow(big_a, 3.0) / 6.0
                + (5.0 - 18.0 * t + t * t + 72.0 * c - 58.0 * self.ep2) * pow(big_a, 5.0) / 120.0)
            + self.false_easting;

        let northing = self.k0
            * ((m - self.m0)
                + n * tan_phi
                    * (big_a * big_a / 2.0
                        + (5.0 - t + 9.0 * c + 4.0 * c * c) * pow(big_a, 4.0) / 24.0
                        + (61.0 - 58.0 * t + t * t + 600.0 * c - 330.0 * self.ep2)
                            * pow(big_a, 6.0)
                            / 720.0))
            + self.false_northing;

        PlanarPoint::new(easting, northing)
    }

    /// The inverse of [`TransverseMercator::forward`].
    #[must_use]
    pub fn inverse(&self, point: PlanarPoint) -> GeoPoint {
        let x = point.x - self.false_easting;
        let y = point.y - self.false_northing;

        let m = self.m0 + y / self.k0;
        let mu = m
            / (self.a
                * (1.0
                    - self.e2 / 4.0
                    - 3.0 * self.e2 * self.e2 / 64.0
                    - 5.0 * pow(self.e2, 3.0) / 256.0));

        let e1 = (1.0 - sqrt(1.0 - self.e2)) / (1.0 + sqrt(1.0 - self.e2));
        let phi1 = mu
            + (3.0 * e1 / 2.0 - 27.0 * pow(e1, 3.0) / 32.0) * sin(2.0 * mu)
            + (21.0 * e1 * e1 / 16.0 - 55.0 * pow(e1, 4.0) / 32.0) * sin(4.0 * mu)
            + (151.0 * pow(e1, 3.0) / 96.0) * sin(6.0 * mu)
            + (1097.0 * pow(e1, 4.0) / 512.0) * sin(8.0 * mu);

        let (sin_phi1, cos_phi1, tan_phi1) = (sin(phi1), cos(phi1), tan(phi1));
        let c1 = self.ep2 * cos_phi1 * cos_phi1;
        let t1 = tan_phi1 * tan_phi1;
        let n1 = self.a / sqrt(1.0 - self.e2 * sin_phi1 * sin_phi1);
        let r1 = self.a * (1.0 - self.e2) / pow(1.0 - self.e2 * sin_phi1 * sin_phi1, 1.5);
        let d = x / (n1 * self.k0);

        let phi = phi1
            - (n1 * tan_phi1 / r1)
                * (d * d / 2.0
                    - (5.0 + 3.0 * t1 + 10.0 * c1 - 4.0 * c1 * c1 - 9.0 * self.ep2) * pow(d, 4.0)
                        / 24.0
                    + (61.0 + 90.0 * t1 + 298.0 * c1 + 45.0 * t1 * t1
                        - 252.0 * self.ep2
                        - 3.0 * c1 * c1)
                        * pow(d, 6.0)
                        / 720.0);

        let lambda = self.lon0_deg.to_radians()
            + (d - (1.0 + 2.0 * t1 + c1) * pow(d, 3.0) / 6.0
                + (5.0 - 2.0 * c1 + 28.0 * t1 - 3.0 * c1 * c1 + 8.0 * self.ep2 + 24.0 * t1 * t1)
                    * pow(d, 5.0)
                    / 120.0)
                / cos_phi1;

        GeoPoint::new(phi.to_degrees(), wrap_deg(lambda.to_degrees()))
    }

    /// The false easting, in meters.
    #[must_use]
    pub fn false_easting(&self) -> f64 {
        self.false_easting
    }

    /// The false northing, in meters.
    #[must_use]
    pub fn false_northing(&self) -> f64 {
        self.false_northing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projection::utm;
    use crate::projection::utm::Zone;

    /// UTM is this projection with origin latitude 0, scale 0.9996 and a 500 km false
    /// easting; the two must agree exactly.
    #[test]
    fn agrees_with_utm_for_a_utm_zone() {
        let zone = Zone::new(18, true);
        let tm = TransverseMercator::new(
            &Ellipsoid::WGS84,
            0.0,
            zone.central_meridian_deg(),
            utm::K0,
            utm::FALSE_EASTING_M,
            0.0,
        );
        let p = GeoPoint::new(40.7128, -74.0060);
        let a = tm.forward(p);
        let b = utm::forward(&Ellipsoid::WGS84, p, zone);
        assert!((a.x - b.x).abs() < 1e-6 && (a.y - b.y).abs() < 1e-6, "{a:?} vs {b:?}");
    }

    /// The origin maps exactly to the false origin, even when the origin latitude is not
    /// the equator (where the northing is offset by the meridional arc).
    #[test]
    fn origin_maps_to_false_origin_at_non_equatorial_latitude() {
        let tm = TransverseMercator::new(
            &Ellipsoid::GRS80,
            49.0,
            -2.0,
            0.9996012717,
            400_000.0,
            -100_000.0,
        );
        let p = tm.forward(GeoPoint::new(49.0, -2.0));
        assert!((p.x - 400_000.0).abs() < 1e-6, "x {}", p.x);
        assert!((p.y + 100_000.0).abs() < 1e-6, "y {}", p.y);
    }

    #[test]
    fn forward_then_inverse_round_trips_at_non_equatorial_origin() {
        let tm = TransverseMercator::new(
            &Ellipsoid::GRS80,
            49.0,
            -2.0,
            0.9996012717,
            400_000.0,
            -100_000.0,
        );
        for &(lat, lon) in &[(51.5074, -0.1278), (50.7, -3.5), (53.8, -1.5)] {
            let back = tm.inverse(tm.forward(GeoPoint::new(lat, lon)));
            assert!((back.lat_deg - lat).abs() < 1e-7, "lat {}", back.lat_deg);
            assert!((back.lon_deg - lon).abs() < 1e-7, "lon {}", back.lon_deg);
        }
    }

    /// Scale on the central meridian is exactly k0: a small northing step on the meridian
    /// is the true arc length times k0.
    #[test]
    fn central_meridian_scale_is_k0() {
        let k0 = 0.9996;
        let tm = TransverseMercator::new(&Ellipsoid::WGS84, 0.0, 9.0, k0, 500_000.0, 0.0);
        let p0 = tm.forward(GeoPoint::new(10.0, 9.0));
        let p1 = tm.forward(GeoPoint::new(10.001, 9.0));
        let grid = p1.y - p0.y;
        let e2 = Ellipsoid::WGS84.eccentricity_squared();
        let s = libm::sin(10.0_f64.to_radians());
        let meridional_radius =
            Ellipsoid::WGS84.semi_major_axis * (1.0 - e2) / libm::pow(1.0 - e2 * s * s, 1.5);
        let ground = meridional_radius * 0.001_f64.to_radians();
        assert!((grid / ground - k0).abs() < 1e-6, "scale {}", grid / ground);
    }
}
