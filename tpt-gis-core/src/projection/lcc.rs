//! Lambert Conformal Conic projection, two standard parallels (Snyder, USGS Professional
//! Paper 1395, §15), on an ellipsoid.
//!
//! The projection is conformal: angles are preserved, and the scale is exactly 1 along
//! both standard parallels. [`LambertConformal::lambert93`] gives the French national
//! grid (EPSG:2154) as a ready-made instance.

use libm::{atan, atan2, cos, log, pow, sin, sqrt, tan};

use crate::{Ellipsoid, GeoPoint, PlanarPoint};

/// Convergence tolerance for the inverse latitude iteration, in radians.
const TOLERANCE: f64 = 1e-12;
/// Iteration cap for the inverse latitude iteration.
const MAX_ITERATIONS: usize = 30;

/// Snyder's `m(φ) = cos φ / √(1 − e² sin² φ)`.
fn m_of(phi: f64, e2: f64) -> f64 {
    let s = sin(phi);
    cos(phi) / sqrt(1.0 - e2 * s * s)
}

/// Snyder's `t(φ) = tan(π/4 − φ/2) / ((1 − e sin φ)/(1 + e sin φ))^(e/2)`.
fn t_of(phi: f64, e: f64) -> f64 {
    let s = sin(phi);
    tan(core::f64::consts::FRAC_PI_4 - phi / 2.0) / pow((1.0 - e * s) / (1.0 + e * s), e / 2.0)
}

/// A Lambert Conformal Conic (two standard parallels) projection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LambertConformal {
    a: f64,
    e: f64,
    e2: f64,
    n: f64,
    /// `a·F`, the scale constant in `ρ = a·F·t^n`.
    a_f: f64,
    rho0: f64,
    lon0: f64,
    false_easting: f64,
    false_northing: f64,
}

impl LambertConformal {
    /// Builds the projection.
    ///
    /// `lat0`/`lon0` is the origin (the point that maps to `(false_easting,
    /// false_northing)`), and `lat1`/`lat2` are the standard parallels in degrees.
    /// `lat1 == lat2` gives the one-parallel form. Angles are in decimal degrees; the
    /// parallels must lie in the same hemisphere and not both be 0 or ±90.
    #[must_use]
    pub fn new(
        ellipsoid: &Ellipsoid,
        lat0_deg: f64,
        lon0_deg: f64,
        lat1_deg: f64,
        lat2_deg: f64,
        false_easting: f64,
        false_northing: f64,
    ) -> Self {
        let a = ellipsoid.semi_major_axis;
        let e2 = ellipsoid.eccentricity_squared();
        let e = sqrt(e2);
        let phi0 = lat0_deg.to_radians();
        let phi1 = lat1_deg.to_radians();
        let phi2 = lat2_deg.to_radians();

        let (m1, t1) = (m_of(phi1, e2), t_of(phi1, e));
        let t2 = t_of(phi2, e);
        let m2 = m_of(phi2, e2);
        // For one standard parallel, n = sin φ1; the two-parallel formula is singular
        // there (0/0), so use the limit.
        let n = if (phi1 - phi2).abs() < 1e-12 {
            sin(phi1)
        } else {
            (log(m1) - log(m2)) / (log(t1) - log(t2))
        };
        let f = m1 / (n * pow(t1, n));
        let a_f = a * f;
        let rho0 = a_f * pow(t_of(phi0, e), n);
        Self { a, e, e2, n, a_f, rho0, lon0: lon0_deg.to_radians(), false_easting, false_northing }
    }

    /// The French national grid, Lambert-93 (EPSG:2154): GRS80, origin 46.5°N 3°E,
    /// standard parallels 44°N and 49°N, false easting 700 000 m, false northing
    /// 6 600 000 m.
    #[must_use]
    pub fn lambert93() -> Self {
        Self::new(&Ellipsoid::GRS80, 46.5, 3.0, 44.0, 49.0, 700_000.0, 6_600_000.0)
    }

    /// The cone constant `n` (between 0 and 1 for a northern-hemisphere cone).
    #[must_use]
    pub fn cone_constant(&self) -> f64 {
        self.n
    }

    /// Projects a geographic coordinate to conic easting/northing in meters.
    #[must_use]
    pub fn forward(&self, point: GeoPoint) -> PlanarPoint {
        let phi = point.lat_rad();
        let rho = self.a_f * pow(t_of(phi, self.e), self.n);
        let theta = self.n * (point.lon_rad() - self.lon0);
        PlanarPoint::new(
            rho * sin(theta) + self.false_easting,
            self.rho0 - rho * cos(theta) + self.false_northing,
        )
    }

    /// The inverse of [`LambertConformal::forward`].
    #[must_use]
    pub fn inverse(&self, point: PlanarPoint) -> GeoPoint {
        let x = point.x - self.false_easting;
        let y = self.rho0 - (point.y - self.false_northing);
        let rho = sqrt(x * x + y * y) * self.n.signum();
        let theta = atan2(x * self.n.signum(), y * self.n.signum());
        let t = pow(rho / self.a_f, 1.0 / self.n);

        // Snyder eq. 7-9: iterate the latitude from the conformal colatitude.
        let mut phi = core::f64::consts::FRAC_PI_2 - 2.0 * atan(t);
        for _ in 0..MAX_ITERATIONS {
            let s = sin(phi);
            let next = core::f64::consts::FRAC_PI_2
                - 2.0 * atan(t * pow((1.0 - self.e * s) / (1.0 + self.e * s), self.e / 2.0));
            let done = (next - phi).abs() < TOLERANCE;
            phi = next;
            if done {
                break;
            }
        }
        let lon = self.lon0 + theta / self.n;
        GeoPoint::new(phi.to_degrees(), lon.to_degrees())
    }

    /// The scale factor along the parallel at `lat_deg`: the projected length of a
    /// small longitude step divided by its true length. Equals 1 on a standard parallel.
    #[must_use]
    pub fn parallel_scale(&self, lat_deg: f64) -> f64 {
        let phi = lat_deg.to_radians();
        let rho = self.a_f * pow(t_of(phi, self.e), self.n);
        rho * self.n / (self.a * m_of(phi, self.e2))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The origin maps exactly to the false origin: a provable invariant of the formulae.
    #[test]
    fn origin_maps_to_false_origin() {
        let lcc =
            LambertConformal::new(&Ellipsoid::GRS80, 46.5, 3.0, 44.0, 49.0, 700_000.0, 6_600_000.0);
        let p = lcc.forward(GeoPoint::new(46.5, 3.0));
        assert!((p.x - 700_000.0).abs() < 1e-6, "x = {}", p.x);
        assert!((p.y - 6_600_000.0).abs() < 1e-6, "y = {}", p.y);
    }

    /// Scale is exactly 1 on both standard parallels: a defining property of the
    /// secant cone, independent of any reference value.
    #[test]
    fn scale_is_unity_on_standard_parallels() {
        let lcc = LambertConformal::lambert93();
        assert!((lcc.parallel_scale(44.0) - 1.0).abs() < 1e-9);
        assert!((lcc.parallel_scale(49.0) - 1.0).abs() < 1e-9);
        // Between the parallels the scale dips below 1; outside it rises above.
        assert!(lcc.parallel_scale(46.5) < 1.0);
        assert!(lcc.parallel_scale(60.0) > 1.0);
    }

    #[test]
    fn forward_then_inverse_round_trips() {
        let lcc = LambertConformal::lambert93();
        for &(lat, lon) in &[(48.8566, 2.3522), (43.2965, 5.3698), (51.0, -4.0)] {
            let original = GeoPoint::new(lat, lon);
            let back = lcc.inverse(lcc.forward(original));
            assert!((back.lat_deg - lat).abs() < 1e-8, "lat {}", back.lat_deg);
            assert!((back.lon_deg - lon).abs() < 1e-8, "lon {}", back.lon_deg);
        }
    }

    /// Paris (2.35°E, 48.86°N) lies north-west of the origin (3°E, 46.5°N), so in
    /// Lambert-93 its easting is below the false easting and its northing above.
    /// Reference point from PROJ (EPSG:2154, via rasterio), in metres.
    #[test]
    fn paris_matches_proj_for_lambert93() {
        let p = LambertConformal::lambert93().forward(GeoPoint::new(48.8566, 2.3522));
        assert!((p.x - 652_469.022_709).abs() < 1.0, "{p:?}");
        assert!((p.y - 6_862_035.259_420).abs() < 1.0, "{p:?}");
    }

    #[test]
    fn paris_is_north_west_of_the_origin() {
        let p = LambertConformal::lambert93().forward(GeoPoint::new(48.8566, 2.3522));
        assert!(p.x < 700_000.0 && p.y > 6_600_000.0, "{p:?}");
    }
}
