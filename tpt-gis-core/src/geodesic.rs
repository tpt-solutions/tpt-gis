//! Geodesic distance, bearing, and destination calculations on an ellipsoid,
//! using Vincenty's formulae (T. Vincenty, 1975).
//!
//! These are iterative, closed-form solutions accurate to sub-millimeter precision
//! for the vast majority of point pairs. The inverse problem fails to converge for
//! a small set of nearly-antipodal points (an acknowledged limitation of Vincenty's
//! original method); such cases return [`GeodesicError::ConvergenceFailure`].

use libm::{atan, atan2, cos, sin, sqrt, tan};

use crate::{Ellipsoid, GeoPoint};

/// Errors produced by geodesic calculations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeodesicError {
    /// Vincenty's iteration did not converge within the iteration limit.
    /// For the inverse problem this occurs only for a small set of nearly-antipodal
    /// point pairs.
    ConvergenceFailure,
    /// An input (coordinate, bearing, or distance) was NaN or infinite, or a latitude
    /// was outside `[-90, 90]` degrees.
    InvalidInput,
}

/// Result of the geodesic inverse problem: the distance and bearings between two points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InverseResult {
    /// Ellipsoidal (geodesic) distance between the two points, in meters.
    pub distance_m: f64,
    /// Initial bearing at the first point, in decimal degrees clockwise from north \[0, 360).
    pub initial_bearing_deg: f64,
    /// Bearing at the second point, in decimal degrees clockwise from north \[0, 360),
    /// continuing in the direction of travel (p1→p2 extended) — *not* the back-bearing
    /// to the first point (which is this value ± 180°).
    pub final_bearing_deg: f64,
}

/// Result of the geodesic direct problem: the destination point and arrival bearing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DirectResult {
    /// The destination point.
    pub destination: GeoPoint,
    /// Bearing at the destination, in decimal degrees clockwise from north \[0, 360).
    pub final_bearing_deg: f64,
}

const MAX_ITERATIONS: u32 = 200;
const CONVERGENCE_THRESHOLD: f64 = 1e-12;

/// Normalizes an angle in radians to the bearing range `[0, 360)` degrees.
fn normalize_bearing_deg(bearing_rad: f64) -> f64 {
    let deg = bearing_rad.to_degrees();
    ((deg % 360.0) + 360.0) % 360.0
}

/// Solves the geodesic inverse problem: given two points, finds the distance and
/// initial/final bearings along the shortest path (geodesic) between them on `ellipsoid`.
///
/// # Errors
/// Returns [`GeodesicError::InvalidInput`] if a coordinate is non-finite or a latitude
/// is outside `[-90, 90]`, and [`GeodesicError::ConvergenceFailure`] if the iteration
/// does not converge within 200 iterations (only expected for nearly-antipodal pairs).
pub fn inverse(
    ellipsoid: &Ellipsoid,
    p1: GeoPoint,
    p2: GeoPoint,
) -> Result<InverseResult, GeodesicError> {
    let valid = |p: GeoPoint| {
        p.lat_deg.is_finite() && p.lon_deg.is_finite() && (-90.0..=90.0).contains(&p.lat_deg)
    };
    if !valid(p1) || !valid(p2) {
        return Err(GeodesicError::InvalidInput);
    }

    if (p1.lat_deg - p2.lat_deg).abs() < 1e-15 && (p1.lon_deg - p2.lon_deg).abs() < 1e-15 {
        return Ok(InverseResult {
            distance_m: 0.0,
            initial_bearing_deg: 0.0,
            final_bearing_deg: 0.0,
        });
    }

    let a = ellipsoid.semi_major_axis;
    let f = ellipsoid.flattening;
    let b = ellipsoid.semi_minor_axis();

    let l = p2.lon_rad() - p1.lon_rad();
    let u1 = atan((1.0 - f) * tan(p1.lat_rad()));
    let u2 = atan((1.0 - f) * tan(p2.lat_rad()));
    let (sin_u1, cos_u1) = (sin(u1), cos(u1));
    let (sin_u2, cos_u2) = (sin(u2), cos(u2));

    let mut lambda = l;
    let mut cos_sq_alpha;
    let mut sin_sigma;
    let mut cos_sigma;
    let mut sigma;
    let mut cos2_sigma_m;

    let mut iterations = 0u32;
    loop {
        let sin_lambda = sin(lambda);
        let cos_lambda = cos(lambda);

        sin_sigma = sqrt(
            (cos_u2 * sin_lambda) * (cos_u2 * sin_lambda)
                + (cos_u1 * sin_u2 - sin_u1 * cos_u2 * cos_lambda)
                    * (cos_u1 * sin_u2 - sin_u1 * cos_u2 * cos_lambda),
        );
        if sin_sigma == 0.0 {
            // Coincident points (already handled above) or numerically identical.
            return Ok(InverseResult {
                distance_m: 0.0,
                initial_bearing_deg: 0.0,
                final_bearing_deg: 0.0,
            });
        }

        cos_sigma = sin_u1 * sin_u2 + cos_u1 * cos_u2 * cos_lambda;
        sigma = atan2(sin_sigma, cos_sigma);

        let sin_alpha = cos_u1 * cos_u2 * sin_lambda / sin_sigma;
        cos_sq_alpha = 1.0 - sin_alpha * sin_alpha;

        cos2_sigma_m = if cos_sq_alpha.abs() < 1e-15 {
            // Equatorial line: cos²α ≈ 0.
            0.0
        } else {
            cos_sigma - 2.0 * sin_u1 * sin_u2 / cos_sq_alpha
        };

        let c = f / 16.0 * cos_sq_alpha * (4.0 + f * (4.0 - 3.0 * cos_sq_alpha));
        let lambda_prev = lambda;
        lambda = l
            + (1.0 - c)
                * f
                * sin_alpha
                * (sigma
                    + c * sin_sigma
                        * (cos2_sigma_m
                            + c * cos_sigma * (-1.0 + 2.0 * cos2_sigma_m * cos2_sigma_m)));

        iterations += 1;
        if (lambda - lambda_prev).abs() < CONVERGENCE_THRESHOLD {
            break;
        }
        if iterations >= MAX_ITERATIONS {
            return Err(GeodesicError::ConvergenceFailure);
        }
    }

    let u_sq = cos_sq_alpha * (a * a - b * b) / (b * b);
    let big_a = 1.0 + u_sq / 16384.0 * (4096.0 + u_sq * (-768.0 + u_sq * (320.0 - 175.0 * u_sq)));
    let big_b = u_sq / 1024.0 * (256.0 + u_sq * (-128.0 + u_sq * (74.0 - 47.0 * u_sq)));
    let delta_sigma = big_b
        * sin_sigma
        * (cos2_sigma_m
            + big_b / 4.0
                * (cos_sigma * (-1.0 + 2.0 * cos2_sigma_m * cos2_sigma_m)
                    - big_b / 6.0
                        * cos2_sigma_m
                        * (-3.0 + 4.0 * sin_sigma * sin_sigma)
                        * (-3.0 + 4.0 * cos2_sigma_m * cos2_sigma_m)));

    let distance_m = b * big_a * (sigma - delta_sigma);

    let sin_lambda = sin(lambda);
    let cos_lambda = cos(lambda);
    let alpha1 = atan2(cos_u2 * sin_lambda, cos_u1 * sin_u2 - sin_u1 * cos_u2 * cos_lambda);
    let alpha2 = atan2(cos_u1 * sin_lambda, -sin_u1 * cos_u2 + cos_u1 * sin_u2 * cos_lambda);

    Ok(InverseResult {
        distance_m,
        initial_bearing_deg: normalize_bearing_deg(alpha1),
        final_bearing_deg: normalize_bearing_deg(alpha2),
    })
}

/// Solves the geodesic direct problem: given a start point, initial bearing, and
/// distance, finds the destination point and the bearing on arrival, on `ellipsoid`.
///
/// The destination longitude is normalised to `[-180, 180)` degrees.
///
/// # Errors
/// Returns [`GeodesicError::InvalidInput`] if any input is NaN or infinite, or if the
/// start latitude is outside `[-90, 90]`. Returns [`GeodesicError::ConvergenceFailure`]
/// if the sigma iteration does not settle within the iteration limit.
pub fn direct(
    ellipsoid: &Ellipsoid,
    start: GeoPoint,
    initial_bearing_deg: f64,
    distance_m: f64,
) -> Result<DirectResult, GeodesicError> {
    let finite = start.lat_deg.is_finite()
        && start.lon_deg.is_finite()
        && initial_bearing_deg.is_finite()
        && distance_m.is_finite();
    if !finite || !(-90.0..=90.0).contains(&start.lat_deg) {
        return Err(GeodesicError::InvalidInput);
    }

    let a = ellipsoid.semi_major_axis;
    let f = ellipsoid.flattening;
    let b = ellipsoid.semi_minor_axis();

    let alpha1 = initial_bearing_deg.to_radians();
    let (sin_alpha1, cos_alpha1) = (sin(alpha1), cos(alpha1));

    let u1 = atan((1.0 - f) * tan(start.lat_rad()));
    let (sin_u1, cos_u1) = (sin(u1), cos(u1));

    let sigma1 = atan2(tan(u1), cos_alpha1);
    let sin_alpha = cos_u1 * sin_alpha1;
    let cos_sq_alpha = 1.0 - sin_alpha * sin_alpha;
    let u_sq = cos_sq_alpha * (a * a - b * b) / (b * b);
    let big_a = 1.0 + u_sq / 16384.0 * (4096.0 + u_sq * (-768.0 + u_sq * (320.0 - 175.0 * u_sq)));
    let big_b = u_sq / 1024.0 * (256.0 + u_sq * (-128.0 + u_sq * (74.0 - 47.0 * u_sq)));

    let base_sigma = distance_m / (b * big_a);
    let mut sigma = base_sigma;
    let mut cos2_sigma_m;
    let mut iterations = 0u32;
    loop {
        cos2_sigma_m = cos(2.0 * sigma1 + sigma);
        let sin_sigma = sin(sigma);
        let cos_sigma = cos(sigma);
        let delta_sigma = big_b
            * sin_sigma
            * (cos2_sigma_m
                + big_b / 4.0
                    * (cos_sigma * (-1.0 + 2.0 * cos2_sigma_m * cos2_sigma_m)
                        - big_b / 6.0
                            * cos2_sigma_m
                            * (-3.0 + 4.0 * sin_sigma * sin_sigma)
                            * (-3.0 + 4.0 * cos2_sigma_m * cos2_sigma_m)));
        let sigma_prev = sigma;
        sigma = base_sigma + delta_sigma;
        iterations += 1;
        if !sigma.is_finite() {
            return Err(GeodesicError::ConvergenceFailure);
        }
        if (sigma - sigma_prev).abs() < CONVERGENCE_THRESHOLD {
            break;
        }
        if iterations >= MAX_ITERATIONS {
            return Err(GeodesicError::ConvergenceFailure);
        }
    }

    let sin_sigma = sin(sigma);
    let cos_sigma = cos(sigma);
    let lat2 = atan2(
        sin_u1 * cos_sigma + cos_u1 * sin_sigma * cos_alpha1,
        (1.0 - f)
            * sqrt(
                sin_alpha * sin_alpha
                    + (sin_u1 * sin_sigma - cos_u1 * cos_sigma * cos_alpha1)
                        * (sin_u1 * sin_sigma - cos_u1 * cos_sigma * cos_alpha1),
            ),
    );
    let lambda =
        atan2(sin_sigma * sin_alpha1, cos_u1 * cos_sigma - sin_u1 * sin_sigma * cos_alpha1);
    let c = f / 16.0 * cos_sq_alpha * (4.0 + f * (4.0 - 3.0 * cos_sq_alpha));
    let l = lambda
        - (1.0 - c)
            * f
            * sin_alpha
            * (sigma
                + c * sin_sigma
                    * (cos2_sigma_m + c * cos_sigma * (-1.0 + 2.0 * cos2_sigma_m * cos2_sigma_m)));

    let lon2 = start.lon_rad() + l;
    let alpha2 = atan2(sin_alpha, -sin_u1 * sin_sigma + cos_u1 * cos_sigma * cos_alpha1);

    Ok(DirectResult {
        destination: GeoPoint::new(lat2.to_degrees(), normalize_longitude_deg(lon2.to_degrees())),
        final_bearing_deg: normalize_bearing_deg(alpha2),
    })
}

/// Wraps a longitude in degrees into `[-180, 180)`.
fn normalize_longitude_deg(lon_deg: f64) -> f64 {
    // `%` on f64 is a core operator (no `std`), unlike `rem_euclid`.
    ((((lon_deg + 180.0) % 360.0) + 360.0) % 360.0) - 180.0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference test vector from Vincenty (1975), widely used to validate implementations:
    /// Flinders Peak to Buninyong, on the WGS84/GRS80 ellipsoid.
    /// Source: Vincenty's original paper; cross-checked against
    /// <https://www.movable-type.co.uk/scripts/latlong-vincenty.html> and
    /// <https://en.wikipedia.org/wiki/Flinders_Peak>.
    ///
    /// Published sources commonly quote the *reverse* azimuth (the back-bearing from
    /// Buninyong to Flinders Peak, 127°10'25.07") rather than α2 as this module defines
    /// it (the forward azimuth *at* the destination, continuing in the p1→p2 direction —
    /// the convention used by Vincenty's own paper, GeographicLib, and movable-type.co.uk).
    /// The two differ by exactly 180°; this test checks against the forward-convention
    /// value (307°10'25.37", i.e. 127°10'25.07" + 180°).
    fn dms_to_deg(deg: f64, min: f64, sec: f64) -> f64 {
        deg + min / 60.0 + sec / 3600.0
    }

    #[test]
    fn vincenty_inverse_flinders_peak_to_buninyong() {
        let flinders_peak =
            GeoPoint::new(-dms_to_deg(37.0, 57.0, 3.720_30), dms_to_deg(144.0, 25.0, 29.524_40));
        let buninyong =
            GeoPoint::new(-dms_to_deg(37.0, 39.0, 10.156_10), dms_to_deg(143.0, 55.0, 35.383_90));

        let result = inverse(&Ellipsoid::WGS84, flinders_peak, buninyong).unwrap();

        assert!(
            (result.distance_m - 54_972.271).abs() < 1e-3,
            "distance_m = {}",
            result.distance_m
        );

        let expected_initial_bearing = dms_to_deg(306.0, 52.0, 5.37);
        let expected_final_bearing = (dms_to_deg(127.0, 10.0, 25.07) + 180.0) % 360.0;
        assert!(
            (result.initial_bearing_deg - expected_initial_bearing).abs() < 1e-4,
            "initial_bearing_deg = {}",
            result.initial_bearing_deg
        );
        assert!(
            (result.final_bearing_deg - expected_final_bearing).abs() < 1e-4,
            "final_bearing_deg = {}",
            result.final_bearing_deg
        );
    }

    #[test]
    fn vincenty_direct_is_inverse_of_inverse() {
        let start = GeoPoint::new(-37.951_033, 144.424_868);
        let bearing = 306.868_158;
        let distance = 54_972.271;

        let result = direct(&Ellipsoid::WGS84, start, bearing, distance).unwrap();

        // Buninyong, approximately.
        assert!((result.destination.lat_deg - (-37.652_821)).abs() < 1e-4);
        assert!((result.destination.lon_deg - 143.926_495).abs() < 1e-4);
    }

    #[test]
    fn inverse_rejects_non_finite_input() {
        let e = &Ellipsoid::WGS84;
        let ok = GeoPoint::new(10.0, 20.0);
        assert_eq!(inverse(e, GeoPoint::new(f64::NAN, 0.0), ok), Err(GeodesicError::InvalidInput));
        assert_eq!(
            inverse(e, ok, GeoPoint::new(0.0, f64::INFINITY)),
            Err(GeodesicError::InvalidInput)
        );
        assert_eq!(inverse(e, ok, GeoPoint::new(91.0, 0.0)), Err(GeodesicError::InvalidInput));
    }

    #[test]
    fn direct_rejects_non_finite_input_without_looping() {
        let start = GeoPoint::new(10.0, 20.0);
        let e = &Ellipsoid::WGS84;
        assert_eq!(direct(e, start, f64::NAN, 1000.0), Err(GeodesicError::InvalidInput));
        assert_eq!(direct(e, start, 45.0, f64::INFINITY), Err(GeodesicError::InvalidInput));
        assert_eq!(
            direct(e, GeoPoint::new(f64::NAN, 0.0), 45.0, 1000.0),
            Err(GeodesicError::InvalidInput)
        );
        assert_eq!(
            direct(e, GeoPoint::new(95.0, 0.0), 45.0, 1000.0),
            Err(GeodesicError::InvalidInput)
        );
    }

    #[test]
    fn direct_normalises_destination_longitude() {
        // Heading due east across the antimeridian must not return a longitude > 180.
        let start = GeoPoint::new(0.0, 179.9);
        let dest = direct(&Ellipsoid::WGS84, start, 90.0, 200_000.0).unwrap().destination;
        assert!(dest.lon_deg >= -180.0 && dest.lon_deg < 180.0, "lon {}", dest.lon_deg);
        assert!(dest.lon_deg < -178.0, "lon {}", dest.lon_deg);
    }

    #[test]
    fn coincident_points_have_zero_distance() {
        let p = GeoPoint::new(45.0, 45.0);
        let result = inverse(&Ellipsoid::WGS84, p, p).unwrap();
        assert_eq!(result.distance_m, 0.0);
    }
}
