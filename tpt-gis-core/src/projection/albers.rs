//! Albers Equal-Area Conic projection, two standard parallels (Snyder, USGS Professional
//! Paper 1395, §14), on an ellipsoid.
//!
//! The projection preserves area: a region's projected area equals its true area on
//! the ellipsoid. Scale therefore differs along meridians and parallels, and the
//! parallel scale is exactly 1 on both standard parallels.

use libm::{asin, atan2, cos, log, sin, sqrt};

use crate::{Ellipsoid, GeoPoint, PlanarPoint};

/// Convergence tolerance for the inverse latitude iteration, in radians.
const TOLERANCE: f64 = 1e-12;
/// Iteration cap for the inverse latitude iteration.
const MAX_ITERATIONS: usize = 30;

/// Snyder's `q(φ)`, the authalic function used by the equal-area conics.
fn q_of(phi: f64, e: f64, e2: f64) -> f64 {
    let s = sin(phi);
    let one_minus = 1.0 - e2 * s * s;
    (1.0 - e2) * (s / one_minus - (1.0 / (2.0 * e)) * log((1.0 - e * s) / (1.0 + e * s)))
}

/// Snyder's `m(φ)`, as used by [`Albers`].
fn m_of(phi: f64, e2: f64) -> f64 {
    let s = sin(phi);
    cos(phi) / sqrt(1.0 - e2 * s * s)
}

/// An Albers Equal-Area Conic projection with two standard parallels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Albers {
    a: f64,
    e: f64,
    e2: f64,
    n: f64,
    c: f64,
    rho0: f64,
    lon0: f64,
    false_easting: f64,
    false_northing: f64,
}

impl Albers {
    /// Builds the projection. `lat0`/`lon0` is the origin, which maps to
    /// `(false_easting, false_northing)`; `lat1`/`lat2` are the standard parallels.
    /// Angles are in decimal degrees. The parallels must not be equal and must not be
    /// symmetric about the equator (the cone constant would be zero).
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

        let m1 = m_of(phi1, e2);
        let m2 = m_of(phi2, e2);
        let q0 = q_of(phi0, e, e2);
        let q1 = q_of(phi1, e, e2);
        let q2 = q_of(phi2, e, e2);
        let n = (m1 * m1 - m2 * m2) / (q2 - q1);
        let c = m1 * m1 + n * q1;
        let rho0 = a * sqrt(c - n * q0) / n;
        Self { a, e, e2, n, c, rho0, lon0: lon0_deg.to_radians(), false_easting, false_northing }
    }

    /// NAD83 / Conus Albers (EPSG:5070): GRS80, origin 23°N 96°W, standard parallels
    /// 29.5°N and 45.5°N, no false origin offset.
    #[must_use]
    pub fn conus_albers() -> Self {
        Self::new(&Ellipsoid::GRS80, 23.0, -96.0, 29.5, 45.5, 0.0, 0.0)
    }

    /// The cone constant `n`.
    #[must_use]
    pub fn cone_constant(&self) -> f64 {
        self.n
    }

    /// Projects a geographic coordinate to equal-area easting/northing in meters.
    #[must_use]
    pub fn forward(&self, point: GeoPoint) -> PlanarPoint {
        let q = q_of(point.lat_rad(), self.e, self.e2);
        let rho = self.a * sqrt(self.c - self.n * q) / self.n;
        let theta = self.n * (point.lon_rad() - self.lon0);
        PlanarPoint::new(
            rho * sin(theta) + self.false_easting,
            self.rho0 - rho * cos(theta) + self.false_northing,
        )
    }

    /// The inverse of [`Albers::forward`].
    #[must_use]
    pub fn inverse(&self, point: PlanarPoint) -> GeoPoint {
        let x = point.x - self.false_easting;
        let y = self.rho0 - (point.y - self.false_northing);
        let rho = sqrt(x * x + y * y);
        let theta = atan2(x, y);
        let q = (self.c - (rho * self.n / self.a) * (rho * self.n / self.a)) / self.n;

        // Snyder eq. 3-16: fixed-point iteration on the authalic latitude.
        let mut phi = asin((q / 2.0).clamp(-1.0, 1.0));
        for _ in 0..MAX_ITERATIONS {
            let s = sin(phi);
            let one_minus = 1.0 - self.e2 * s * s;
            let correction = one_minus * one_minus / (2.0 * cos(phi))
                * (q / (1.0 - self.e2) - s / one_minus
                    + (1.0 / (2.0 * self.e)) * log((1.0 - self.e * s) / (1.0 + self.e * s)));
            let next = phi + correction;
            let done = (next - phi).abs() < TOLERANCE;
            phi = next;
            if done {
                break;
            }
        }
        let lon = self.lon0 + theta / self.n;
        GeoPoint::new(phi.to_degrees(), lon.to_degrees())
    }

    /// Scale factor along the parallel at `lat_deg`. Equals 1 on a standard parallel.
    #[must_use]
    pub fn parallel_scale(&self, lat_deg: f64) -> f64 {
        let phi = lat_deg.to_radians();
        let q = q_of(phi, self.e, self.e2);
        let rho = self.a * sqrt(self.c - self.n * q) / self.n;
        rho * self.n / (self.a * m_of(phi, self.e2))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Ellipsoid;

    /// USGS conterminous-US Albers parameters (NAD83): origin 23°N 96°W, standard
    /// parallels 29.5°N and 45.5°N.
    fn conus() -> Albers {
        Albers::conus_albers()
    }

    /// Reference points from PROJ (EPSG:5070, via rasterio), in metres.
    #[test]
    fn matches_proj_for_conus_albers() {
        let cases = [
            ((39.74, -104.99), -762_375.214_768, 1_893_930.439_596),
            ((25.76, -80.19), 1_594_288.146_038, 434_316.239_231),
            ((37.77, -122.42), -2_275_632.970_779, 1_955_421.688_045),
        ];
        for ((lat, lon), x, y) in cases {
            let p = conus().forward(GeoPoint::new(lat, lon));
            assert!((p.x - x).abs() < 1.0 && (p.y - y).abs() < 1.0, "{lat},{lon}: {p:?}");
        }
    }

    #[test]
    fn origin_maps_to_false_origin() {
        let p = conus().forward(GeoPoint::new(23.0, -96.0));
        assert!(p.x.abs() < 1e-6 && p.y.abs() < 1e-6, "{p:?}");
    }

    #[test]
    fn scale_is_unity_on_standard_parallels() {
        let albers = conus();
        assert!((albers.parallel_scale(29.5) - 1.0).abs() < 1e-9);
        assert!((albers.parallel_scale(45.5) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn forward_then_inverse_round_trips() {
        let albers = conus();
        for &(lat, lon) in &[(39.0, -77.0), (47.6, -122.3), (25.8, -80.2)] {
            let back = albers.inverse(albers.forward(GeoPoint::new(lat, lon)));
            assert!((back.lat_deg - lat).abs() < 1e-8, "lat {}", back.lat_deg);
            assert!((back.lon_deg - lon).abs() < 1e-8, "lon {}", back.lon_deg);
        }
    }

    /// Equal-area: a small square of ground maps to a square of the same area. Compare
    /// the planar area of a 0.1°×0.1° cell with its true ellipsoidal area.
    #[test]
    fn small_cell_preserves_area() {
        let albers = conus();
        let (lat, lon, d) = (38.0, -100.0, 0.1_f64);
        let corners = [
            GeoPoint::new(lat, lon),
            GeoPoint::new(lat, lon + d),
            GeoPoint::new(lat + d, lon + d),
            GeoPoint::new(lat + d, lon),
        ];
        let p: [PlanarPoint; 4] = corners.map(|c| albers.forward(c));
        let mut planar = 0.0;
        for i in 0..4 {
            let (a, b) = (p[i], p[(i + 1) % 4]);
            planar += a.x * b.y - b.x * a.y;
        }
        let planar = (planar / 2.0).abs();
        // True area of the cell: ∫∫ N·M ... ≈ (d in radians)² · N·M·cos φ (ellipsoidal).
        let e2 = Ellipsoid::GRS80.eccentricity_squared();
        let s = sin(lat.to_radians());
        let w = sqrt(1.0 - e2 * s * s);
        let n_rad = Ellipsoid::GRS80.semi_major_axis / w;
        let m_rad = Ellipsoid::GRS80.semi_major_axis * (1.0 - e2) / (w * w * w);
        let true_area = d.to_radians() * n_rad * cos(lat.to_radians()) * d.to_radians() * m_rad;
        assert!((planar / true_area - 1.0).abs() < 1e-3, "planar {planar} vs true {true_area}");
    }
}
