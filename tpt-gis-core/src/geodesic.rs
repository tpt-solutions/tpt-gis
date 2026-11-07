//! Geodesic distance, bearing, and destination calculations on an ellipsoid, using
//! Karney's algorithm (C. F. F. Karney, "Algorithms for geodesics", J. Geodesy 87,
//! 43–55, 2013).
//!
//! This is a port of the solver in GeographicLib (Copyright (c) Charles Karney, MIT/X11
//! licence, <https://geographiclib.sourceforge.io/>), reduced to what this module returns:
//! distance and azimuths for the inverse problem, and destination and azimuths for the
//! direct problem. The series coefficients are GeographicLib's, generated from its
//! reference source. The method converges for every pair of points, including the
//! nearly antipodal pairs where Vincenty's iteration fails.
//!
//! Angles in the solver are in degrees where the reference uses degrees (latitudes,
//! longitudes, azimuths in the `*d` helpers) and radians elsewhere, as in the reference.

use libm::{atan, atan2, atanh, copysign, cos, fabs, fmod, hypot, remainder, rint, sin, sqrt};

use crate::{Ellipsoid, GeoPoint};

/// Errors produced by geodesic calculations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeodesicError {
    /// The inverse iteration did not converge. Karney's method converges for every pair of
    /// valid points, so this is a defensive error rather than an expected one.
    ConvergenceFailure,
    /// An input (coordinate, bearing, or distance) was NaN or infinite, or a latitude
    /// was outside `[-90, 90]` degrees, or the ellipsoid is degenerate.
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

/// Series order (GeographicLib's `GEOGRAPHICLIB_GEODESIC_ORDER`).
const ORDER: usize = 6;
/// Iteration limits of the inverse solver (GeographicLib's `maxit1_` and `maxit2_`).
const MAXIT1: u32 = 20;
const MAXIT2: u32 = MAXIT1 + 53 + 10;

const TOL0: f64 = f64::EPSILON;
const TOL1: f64 = 200.0 * TOL0;

/// The smallest positive normal, square-rooted: GeographicLib's `tiny_`.
fn tiny() -> f64 {
    sqrt(f64::MIN_POSITIVE)
}

fn tol2() -> f64 {
    sqrt(TOL0)
}

fn xthresh() -> f64 {
    1000.0 * tol2()
}

// ----------------------------------------------------------------------------
// Series coefficients (generated from GeographicLib's geodesic.py).
// ----------------------------------------------------------------------------

const A1M1_COEFF: [f64; 5] = [1.0, 4.0, 64.0, 0.0, 256.0];

const C1_COEFF: [f64; 18] = [
    -1.0, 6.0, -16.0, 32.0, -9.0, 64.0, -128.0, 2048.0, 9.0, -16.0, 768.0, 3.0, -5.0, 512.0, -7.0,
    1280.0, -7.0, 2048.0,
];

const C1P_COEFF: [f64; 18] = [
    205.0, -432.0, 768.0, 1536.0, 4005.0, -4736.0, 3840.0, 12288.0, -225.0, 116.0, 384.0, -7173.0,
    2695.0, 7680.0, 3467.0, 7680.0, 38081.0, 61440.0,
];

const A2M1_COEFF: [f64; 5] = [-11.0, -28.0, -192.0, 0.0, 256.0];

const C2_COEFF: [f64; 18] = [
    1.0, 2.0, 16.0, 32.0, 35.0, 64.0, 384.0, 2048.0, 15.0, 80.0, 768.0, 7.0, 35.0, 512.0, 63.0,
    1280.0, 77.0, 2048.0,
];

const A3_COEFF: [f64; 18] = [
    -3.0, 128.0, -2.0, -3.0, 64.0, -1.0, -3.0, -1.0, 16.0, 3.0, -1.0, -2.0, 8.0, 1.0, -1.0, 2.0,
    1.0, 1.0,
];

const C3_COEFF: [f64; 45] = [
    3.0, 128.0, 2.0, 5.0, 128.0, -1.0, 3.0, 3.0, 64.0, -1.0, 0.0, 1.0, 8.0, -1.0, 1.0, 4.0, 5.0,
    256.0, 1.0, 3.0, 128.0, -3.0, -2.0, 3.0, 64.0, 1.0, -3.0, 2.0, 32.0, 7.0, 512.0, -10.0, 9.0,
    384.0, 5.0, -9.0, 5.0, 192.0, 7.0, 512.0, -14.0, 7.0, 512.0, 21.0, 2560.0,
];

const C4_COEFF: [f64; 77] = [
    97.0, 15015.0, 1088.0, 156.0, 45045.0, -224.0, -4784.0, 1573.0, 45045.0, -10656.0, 14144.0,
    -4576.0, -858.0, 45045.0, 64.0, 624.0, -4576.0, 6864.0, -3003.0, 15015.0, 100.0, 208.0, 572.0,
    3432.0, -12012.0, 30030.0, 45045.0, 1.0, 9009.0, -2944.0, 468.0, 135135.0, 5792.0, 1040.0,
    -1287.0, 135135.0, 5952.0, -11648.0, 9152.0, -2574.0, 135135.0, -64.0, -624.0, 4576.0, -6864.0,
    3003.0, 135135.0, 8.0, 10725.0, 1856.0, -936.0, 225225.0, -8448.0, 4992.0, -1144.0, 225225.0,
    -1440.0, 4160.0, -4576.0, 1716.0, 225225.0, -136.0, 63063.0, 1024.0, -208.0, 105105.0, 3584.0,
    -3328.0, 1144.0, 315315.0, -128.0, 135135.0, -2560.0, 832.0, 405405.0, 128.0, 99099.0,
];

// ----------------------------------------------------------------------------
// Small helpers, following GeographicLib's Math class.
// ----------------------------------------------------------------------------

fn sq(x: f64) -> f64 {
    x * x
}

/// Polynomial `p[s] x^N + ... + p[s+N]` by Horner's rule.
fn polyval(n: usize, p: &[f64], s: usize, x: f64) -> f64 {
    let mut y = p[s];
    for i in 1..=n {
        y = y * x + p[s + i];
    }
    y
}

/// Clenshaw summation of a sine series (`sinp`) or a cosine series.
fn sin_cos_series(sinp: bool, sinx: f64, cosx: f64, c: &[f64]) -> f64 {
    let mut k = c.len();
    let mut n = k - usize::from(sinp);
    let ar = 2.0 * (cosx - sinx) * (cosx + sinx);
    let mut y0 = 0.0;
    let mut y1 = 0.0;
    if n & 1 == 1 {
        k -= 1;
        y0 = c[k];
    }
    n /= 2;
    for _ in 0..n {
        k -= 1;
        y1 = ar * y0 - y1 + c[k];
        k -= 1;
        y0 = ar * y1 - y0 + c[k];
    }
    if sinp {
        2.0 * sinx * cosx * y0
    } else {
        cosx * (y0 - y1)
    }
}

/// Real cube root, with the sign of `x`.
fn cbrt_signed(x: f64) -> f64 {
    copysign(libm::pow(fabs(x), 1.0 / 3.0), x)
}

/// Solves the astroid equation `k^4 + 2k^3 - (x^2 + y^2 - 1)k^2 - 2y^2 k - y^2 = 0` for its
/// positive root.
fn astroid(x: f64, y: f64) -> f64 {
    let p = sq(x);
    let q = sq(y);
    let r = (p + q - 1.0) / 6.0;
    if !(q == 0.0 && r <= 0.0) {
        let s = p * q / 4.0;
        let r2 = sq(r);
        let r3 = r * r2;
        let disc = s * (s + 2.0 * r3);
        let mut u = r;
        if disc >= 0.0 {
            let mut t3 = s + r3;
            t3 += if t3 < 0.0 { -sqrt(disc) } else { sqrt(disc) };
            let t = cbrt_signed(t3);
            u += t + if t != 0.0 { r2 / t } else { 0.0 };
        } else {
            let ang = atan2(sqrt(-disc), -(s + r3));
            u += 2.0 * r * cos(ang / 3.0);
        }
        let v = sqrt(sq(u) + q);
        let uv = if u < 0.0 { q / (v - u) } else { u + v };
        let w = (uv - q) / (2.0 * v);
        uv / (sqrt(uv + sq(w)) + w)
    } else {
        0.0
    }
}

/// Normalises a two-vector.
fn norm2(x: f64, y: f64) -> (f64, f64) {
    let r = hypot(x, y);
    (x / r, y / r)
}

/// Rounds an angle in degrees so that tiny values become exactly zero or stay on the
/// correct side of the grid near `1/16`.
fn ang_round(x: f64) -> f64 {
    let z = 1.0 / 16.0;
    let mut y = fabs(x);
    if y < z {
        y = z - (z - y);
    }
    copysign(y, x)
}

/// Error-free transformation of a sum: returns `(s, t)` with `s + t = u + v` exactly.
fn error_free_sum(u: f64, v: f64) -> (f64, f64) {
    let s = u + v;
    let mut up = s - v;
    let mut vpp = s - up;
    up -= u;
    vpp -= v;
    let t = if s == 0.0 { s } else { 0.0 - (up + vpp) };
    (s, t)
}

fn remainder_deg(x: f64, y: f64) -> f64 {
    if x.is_finite() {
        remainder(x, y)
    } else {
        f64::NAN
    }
}

/// Reduces an angle in degrees to `[-180, 180]`.
fn ang_normalize(x: f64) -> f64 {
    let y = remainder_deg(x, 360.0);
    if fabs(y) == 180.0 {
        copysign(180.0, x)
    } else {
        y
    }
}

/// `y - x` in degrees, reduced to `[-180, 180]`, with the error term `t`.
fn ang_diff(x: f64, y: f64) -> (f64, f64) {
    let (d, t) = error_free_sum(remainder_deg(-x, 360.0), remainder_deg(y, 360.0));
    let (mut d, t) = error_free_sum(remainder_deg(d, 360.0), t);
    if d == 0.0 || fabs(d) == 180.0 {
        d = copysign(d, if t == 0.0 { y - x } else { -t });
    }
    (d, t)
}

/// Sine and cosine of an angle in degrees, exact at multiples of 90°.
fn sincosd(x: f64) -> (f64, f64) {
    let r = if x.is_finite() { fmod(x, 360.0) } else { f64::NAN };
    let q: i64 = if r.is_nan() { 0 } else { rint(r / 90.0) as i64 };
    let r = (r - 90.0 * q as f64).to_radians();
    sincos_quadrant(r, q, x)
}

/// Sine and cosine of `x + t` in degrees, with `x` in `[-180, 180]`.
fn sincosde(x: f64, t: f64) -> (f64, f64) {
    let q: i64 = if x.is_finite() { rint(x / 90.0) as i64 } else { 0 };
    let r = ang_round((x - 90.0 * q as f64) + t).to_radians();
    sincos_quadrant(r, q, x)
}

/// Applies the quadrant `q` to the sine and cosine of `r` (in radians); `x` decides the
/// sign of a zero sine.
fn sincos_quadrant(r: f64, q: i64, x: f64) -> (f64, f64) {
    let (mut s, mut c) = (sin(r), cos(r));
    match q.rem_euclid(4) {
        1 => (s, c) = (c, -s),
        2 => (s, c) = (-s, -c),
        3 => (s, c) = (-c, s),
        _ => {}
    }
    if s == 0.0 {
        s = copysign(s, x);
    }
    (s, c)
}

/// `atan2(y, x)` in degrees, computed so that exact multiples of 90° are exact.
fn atan2d(y: f64, x: f64) -> f64 {
    let (y, mut x, q) = if fabs(y) > fabs(x) { (x, y, 2) } else { (y, x, 0) };
    let mut q = q;
    if x < 0.0 {
        q += 1;
        x = -x;
    }
    let ang = atan2(y, x).to_degrees();
    match q {
        1 => copysign(180.0, y) - ang,
        2 => 90.0 - ang,
        3 => -90.0 + ang,
        _ => ang,
    }
}

/// `x` if it is a positive number, otherwise `0`, as in `max(0, x) + 0`.
fn max_zero(x: f64) -> f64 {
    if x > 0.0 {
        x
    } else {
        0.0
    }
}

// ----------------------------------------------------------------------------
// The solver.
// ----------------------------------------------------------------------------

/// The ellipsoid constants and series coefficients for one ellipsoid.
#[derive(Debug, Clone, Copy)]
struct Karney {
    a: f64,
    f: f64,
    f1: f64,
    e2: f64,
    ep2: f64,
    n: f64,
    b: f64,
    c2: f64,
    etol2: f64,
    a3x: [f64; ORDER],
    c3x: [f64; 15],
    c4x: [f64; 21],
}

/// The solution of the inverse problem, before the public functions shape it.
struct InverseSolution {
    distance_m: f64,
    area_m2: f64,
    salp1: f64,
    calp1: f64,
    salp2: f64,
    calp2: f64,
}

/// The starting point of the inverse iteration.
struct InverseStart {
    sig12: f64,
    salp1: f64,
    calp1: f64,
    salp2: f64,
    calp2: f64,
    dnm: f64,
}

/// The values of one evaluation of the longitude difference.
struct Lambda {
    lam12: f64,
    salp2: f64,
    calp2: f64,
    sig12: f64,
    ssig1: f64,
    csig1: f64,
    ssig2: f64,
    csig2: f64,
    eps: f64,
    domg12: f64,
    dlam12: f64,
}

impl Karney {
    fn new(ellipsoid: &Ellipsoid) -> Result<Self, GeodesicError> {
        let a = ellipsoid.semi_major_axis;
        let f = ellipsoid.flattening;
        let f1 = 1.0 - f;
        let b = a * f1;
        if !(a.is_finite() && a > 0.0 && b.is_finite() && b > 0.0 && f.is_finite() && f < 1.0) {
            return Err(GeodesicError::InvalidInput);
        }
        let e2 = f * (2.0 - f);
        let ep2 = e2 / sq(f1);
        let n = f / (2.0 - f);
        let etol2 = 0.1 * tol2() / sqrt(fmax(0.001, fabs(f)) * fmin(1.0, 1.0 - f / 2.0) / 2.0);
        // The authalic radius squared, c² = (a² + b² atanh(e)/e) / 2 (or its e² < 0 form).
        let ratio = if e2 == 0.0 {
            1.0
        } else if e2 > 0.0 {
            atanh(sqrt(e2)) / sqrt(fabs(e2))
        } else {
            atan(sqrt(-e2)) / sqrt(fabs(e2))
        };
        let c2 = (sq(a) + sq(b) * ratio) / 2.0;
        Ok(Karney {
            a,
            f,
            f1,
            e2,
            ep2,
            n,
            b,
            c2,
            etol2,
            a3x: a3_coeffs(n),
            c3x: c3_coeffs(n),
            c4x: c4_coeffs(n),
        })
    }

    /// The `C4` series coefficients `c[0..6]` at `eps`.
    fn c4f(&self, eps: f64) -> [f64; ORDER] {
        let mut c = [0.0; ORDER];
        let mut mult = 1.0;
        let mut o = 0;
        for (l, slot) in c.iter_mut().enumerate() {
            let m = ORDER - l - 1;
            *slot = mult * polyval(m, &self.c4x, o, eps);
            o += m + 1;
            mult *= eps;
        }
        c
    }

    fn a3f(&self, eps: f64) -> f64 {
        polyval(ORDER - 1, &self.a3x, 0, eps)
    }

    /// The `C3` series coefficients `c[1..6]` at `eps`.
    fn c3f(&self, eps: f64) -> [f64; ORDER] {
        let mut c = [0.0; ORDER];
        let mut mult = 1.0;
        let mut o = 0;
        for (l, slot) in c.iter_mut().enumerate().skip(1) {
            let m = ORDER - l - 1;
            mult *= eps;
            *slot = mult * polyval(m, &self.c3x, o, eps);
            o += m + 1;
        }
        c
    }

    /// Distance and reduced length at the given arc, both scaled by `1/b`-free terms as in
    /// the reference: returns `(s12b, m12b, m0)`.
    #[allow(clippy::too_many_arguments)]
    fn lengths(
        &self,
        eps: f64,
        sig12: f64,
        ssig1: f64,
        csig1: f64,
        dn1: f64,
        ssig2: f64,
        csig2: f64,
        dn2: f64,
    ) -> (f64, f64, f64) {
        let c1a = c1_series(eps);
        let c2a = c2_series(eps);
        let a1m1 = a1m1f(eps);
        let a2m1 = a2m1f(eps);
        let m0x = a1m1 - a2m1;
        let a1 = 1.0 + a1m1;
        let a2 = 1.0 + a2m1;
        let b1 =
            sin_cos_series(true, ssig2, csig2, &c1a) - sin_cos_series(true, ssig1, csig1, &c1a);
        let s12b = a1 * (sig12 + b1);
        let b2 =
            sin_cos_series(true, ssig2, csig2, &c2a) - sin_cos_series(true, ssig1, csig1, &c2a);
        let j12 = m0x * sig12 + (a1 * b1 - a2 * b2);
        let m12b = dn2 * (csig1 * ssig2) - dn1 * (ssig1 * csig2) - csig1 * csig2 * j12;
        (s12b, m12b, m0x)
    }

    /// A starting point for Newton's method (or the answer, for nearly coincident points).
    #[allow(clippy::too_many_arguments)]
    fn inverse_start(
        &self,
        sbet1: f64,
        cbet1: f64,
        dn1: f64,
        sbet2: f64,
        cbet2: f64,
        dn2: f64,
        lam12: f64,
        slam12: f64,
        clam12: f64,
    ) -> InverseStart {
        let mut sig12 = -1.0;
        let mut salp2 = f64::NAN;
        let mut calp2 = f64::NAN;
        let mut dnm = f64::NAN;
        let sbet12 = sbet2 * cbet1 - cbet2 * sbet1;
        let cbet12 = cbet2 * cbet1 + sbet2 * sbet1;
        let sbet12a = sbet2 * cbet1 + cbet2 * sbet1;
        let shortline = cbet12 >= 0.0 && sbet12 < 0.5 && cbet2 * lam12 < 0.5;
        let (somg12, comg12) = if shortline {
            let mut sbetm2 = sq(sbet1 + sbet2);
            sbetm2 /= sbetm2 + sq(cbet1 + cbet2);
            dnm = sqrt(1.0 + self.ep2 * sbetm2);
            let omg12 = lam12 / (self.f1 * dnm);
            (sin(omg12), cos(omg12))
        } else {
            (slam12, clam12)
        };
        let mut salp1 = cbet2 * somg12;
        let mut calp1 = if comg12 >= 0.0 {
            sbet12 + cbet2 * sbet1 * sq(somg12) / (1.0 + comg12)
        } else {
            sbet12a - cbet2 * sbet1 * sq(somg12) / (1.0 - comg12)
        };
        let ssig12 = hypot(salp1, calp1);
        let csig12 = sbet1 * sbet2 + cbet1 * cbet2 * comg12;
        if shortline && ssig12 < self.etol2 {
            // Really short lines.
            salp2 = cbet1 * somg12;
            calp2 = sbet12
                - cbet1
                    * sbet2
                    * (if comg12 >= 0.0 { sq(somg12) / (1.0 + comg12) } else { 1.0 - comg12 });
            (salp2, calp2) = norm2(salp2, calp2);
            sig12 = atan2(ssig12, csig12);
        } else if fabs(self.n) >= 0.1
            || csig12 >= 0.0
            || ssig12 >= 6.0 * fabs(self.n) * core::f64::consts::PI * sq(cbet1)
        {
            // The zeroth-order spherical approximation is good enough.
        } else {
            // Scale to coordinates where the antipodal point is at the origin and the
            // singular point is at y = 0, x = -1.
            let lam12x = atan2(-slam12, -clam12);
            let (x, y, lamscale);
            if self.f >= 0.0 {
                let k2 = sq(sbet1) * self.ep2;
                let eps = k2 / (2.0 * (1.0 + sqrt(1.0 + k2)) + k2);
                lamscale = self.f * cbet1 * self.a3f(eps) * core::f64::consts::PI;
                let betscale = lamscale * cbet1;
                x = lam12x / lamscale;
                y = sbet12a / betscale;
            } else {
                let cbet12a = cbet2 * cbet1 - sbet2 * sbet1;
                let bet12a = atan2(sbet12a, cbet12a);
                let (_, m12b, m0) = self.lengths(
                    self.n,
                    core::f64::consts::PI + bet12a,
                    sbet1,
                    -cbet1,
                    dn1,
                    sbet2,
                    cbet2,
                    dn2,
                );
                x = -1.0 + m12b / (cbet1 * cbet2 * m0 * core::f64::consts::PI);
                let betscale = if x < -0.01 {
                    sbet12a / x
                } else {
                    -self.f * sq(cbet1) * core::f64::consts::PI
                };
                lamscale = betscale / cbet1;
                y = lam12x / lamscale;
            }
            if y > -TOL1 && x > -1.0 - xthresh() {
                // Strip near the cut.
                if self.f >= 0.0 {
                    salp1 = fmin(1.0, -x);
                    calp1 = -sqrt(1.0 - sq(salp1));
                } else {
                    calp1 = fmax(if x > -TOL1 { 0.0 } else { -1.0 }, x);
                    salp1 = sqrt(1.0 - sq(calp1));
                }
            } else {
                // Estimate omg12 from the astroid, then use the spherical formula for alp1.
                let k = astroid(x, y);
                let omg12a = lamscale
                    * (if self.f >= 0.0 { -x * k / (1.0 + k) } else { -y * (1.0 + k) / k });
                let somg12 = sin(omg12a);
                let comg12 = -cos(omg12a);
                salp1 = cbet2 * somg12;
                calp1 = sbet12a - cbet2 * sbet1 * sq(somg12) / (1.0 - comg12);
            }
        }
        // Sanity check on the starting guess. A NaN passes through the check.
        // NaN is not `<= 0`, so it takes the first branch, as in the reference.
        if salp1 > 0.0 || salp1.is_nan() {
            (salp1, calp1) = norm2(salp1, calp1);
        } else {
            salp1 = 1.0;
            calp1 = 0.0;
        }
        InverseStart { sig12, salp1, calp1, salp2, calp2, dnm }
    }

    /// The difference in longitude for a given azimuth at the first point, solving the
    /// hybrid problem.
    #[allow(clippy::too_many_arguments)]
    fn lambda12(
        &self,
        sbet1: f64,
        cbet1: f64,
        dn1: f64,
        sbet2: f64,
        cbet2: f64,
        dn2: f64,
        salp1: f64,
        calp1: f64,
        slam120: f64,
        clam120: f64,
        diffp: bool,
    ) -> Lambda {
        let calp1 = if sbet1 == 0.0 && calp1 == 0.0 { -tiny() } else { calp1 };
        let salp0 = salp1 * cbet1;
        let calp0 = hypot(calp1, salp1 * sbet1);
        let mut ssig1 = sbet1;
        let somg1 = salp0 * sbet1;
        let mut csig1 = calp1 * cbet1;
        let comg1 = csig1;
        (ssig1, csig1) = norm2(ssig1, csig1);
        let salp2 = if cbet2 != cbet1 { salp0 / cbet2 } else { salp1 };
        let calp2 = if cbet2 != cbet1 || fabs(sbet2) != -sbet1 {
            sqrt(
                sq(calp1 * cbet1)
                    + if cbet1 < -sbet1 {
                        (cbet2 - cbet1) * (cbet1 + cbet2)
                    } else {
                        (sbet1 - sbet2) * (sbet1 + sbet2)
                    },
            ) / cbet2
        } else {
            fabs(calp1)
        };
        let mut ssig2 = sbet2;
        let somg2 = salp0 * sbet2;
        let mut csig2 = calp2 * cbet2;
        let comg2 = csig2;
        (ssig2, csig2) = norm2(ssig2, csig2);
        let sig12 = atan2(max_zero(csig1 * ssig2 - ssig1 * csig2), csig1 * csig2 + ssig1 * ssig2);
        let somg12 = max_zero(comg1 * somg2 - somg1 * comg2);
        let comg12 = comg1 * comg2 + somg1 * somg2;
        let eta = atan2(somg12 * clam120 - comg12 * slam120, comg12 * clam120 + somg12 * slam120);
        let k2 = sq(calp0) * self.ep2;
        let eps = k2 / (2.0 * (1.0 + sqrt(1.0 + k2)) + k2);
        let c3a = self.c3f(eps);
        let b312 =
            sin_cos_series(true, ssig2, csig2, &c3a) - sin_cos_series(true, ssig1, csig1, &c3a);
        let domg12 = -self.f * self.a3f(eps) * salp0 * (sig12 + b312);
        let lam12 = eta + domg12;
        let dlam12 = if diffp {
            if calp2 == 0.0 {
                -2.0 * self.f1 * dn1 / sbet1
            } else {
                let (_, m12b, _) = self.lengths(eps, sig12, ssig1, csig1, dn1, ssig2, csig2, dn2);
                m12b * (self.f1 / (calp2 * cbet2))
            }
        } else {
            f64::NAN
        };
        Lambda { lam12, salp2, calp2, sig12, ssig1, csig1, ssig2, csig2, eps, domg12, dlam12 }
    }

    /// The inverse problem: `(s12, salp1, calp1, salp2, calp2)`, where the sines and cosines
    /// of the azimuths are already signed for the original (unswapped) points.
    fn inverse(
        &self,
        lat1: f64,
        lon1: f64,
        lat2: f64,
        lon2: f64,
    ) -> Result<InverseSolution, GeodesicError> {
        let (lon12_raw, lon12s_raw) = ang_diff(lon1, lon2);
        let mut lonsign = copysign(1.0, lon12_raw);
        let lon12 = lonsign * lon12_raw;
        let lon12s = lonsign * lon12s_raw;
        let lam12 = lon12.to_radians();
        let (slam12, clam12) = sincosde(lon12, lon12s);
        // The supplementary longitude difference.
        let lon12s = (180.0 - lon12) - lon12s;

        let mut lat1 = ang_round(lat1);
        let mut lat2 = ang_round(lat2);
        let swapp = if fabs(lat1) < fabs(lat2) || lat2.is_nan() { -1.0 } else { 1.0 };
        if swapp < 0.0 {
            lonsign = -lonsign;
            core::mem::swap(&mut lat1, &mut lat2);
        }
        let latsign = copysign(1.0, -lat1);
        lat1 *= latsign;
        lat2 *= latsign;

        let (mut sbet1, mut cbet1) = sincosd(lat1);
        sbet1 *= self.f1;
        (sbet1, cbet1) = norm2(sbet1, cbet1);
        cbet1 = fmax(tiny(), cbet1);
        let (mut sbet2, mut cbet2) = sincosd(lat2);
        sbet2 *= self.f1;
        (sbet2, cbet2) = norm2(sbet2, cbet2);
        cbet2 = fmax(tiny(), cbet2);

        if cbet1 < -sbet1 {
            if cbet2 == cbet1 {
                sbet2 = copysign(sbet1, sbet2);
            }
        } else if fabs(sbet2) == -sbet1 {
            cbet2 = cbet1;
        }
        let dn1 = sqrt(1.0 + self.ep2 * sq(sbet1));
        let dn2 = sqrt(1.0 + self.ep2 * sq(sbet2));

        let mut meridian = lat1 == -90.0 || slam12 == 0.0;
        let mut s12x = 0.0;
        // sin and cos of the longitude difference on the auxiliary sphere, when known.
        let mut omg: Option<(f64, f64)> = None;
        let (mut salp1, mut calp1, mut salp2, mut calp2) = (0.0, 0.0, 0.0, 0.0);
        if meridian {
            calp1 = clam12;
            salp1 = slam12;
            calp2 = 1.0;
            salp2 = 0.0;
            let ssig1 = sbet1;
            let csig1 = calp1 * cbet1;
            let ssig2 = sbet2;
            let csig2 = calp2 * cbet2;
            let sig12 =
                atan2(max_zero(csig1 * ssig2 - ssig1 * csig2), csig1 * csig2 + ssig1 * ssig2);
            let (mut s12, m12, _) =
                self.lengths(self.n, sig12, ssig1, csig1, dn1, ssig2, csig2, dn2);
            if sig12 < tol2() || m12 >= 0.0 {
                if sig12 < 3.0 * tiny() || (sig12 < TOL0 && (s12 < 0.0 || m12 < 0.0)) {
                    s12 = 0.0;
                }
                s12x = s12 * self.b;
            } else {
                meridian = false;
            }
        }

        if !meridian && sbet1 == 0.0 && (self.f <= 0.0 || lon12s >= self.f * 180.0) {
            // An equatorial line between points on the equator.
            calp1 = 0.0;
            calp2 = 0.0;
            salp1 = 1.0;
            salp2 = 1.0;
            s12x = self.a * lam12;
            let omg12 = lam12 / self.f1;
            omg = Some((sin(omg12), cos(omg12)));
        } else if !meridian {
            let start =
                self.inverse_start(sbet1, cbet1, dn1, sbet2, cbet2, dn2, lam12, slam12, clam12);
            salp1 = start.salp1;
            calp1 = start.calp1;
            salp2 = start.salp2;
            calp2 = start.calp2;
            if start.sig12 >= 0.0 {
                // Really short, or near-antipodal line handled by the starting guess.
                let dnm = start.dnm;
                s12x = start.sig12 * self.b * dnm;
                let omg12 = lam12 / (self.f1 * dnm);
                omg = Some((sin(omg12), cos(omg12)));
            } else {
                let mut numit: u32 = 0;
                let mut tripn = false;
                let mut tripb = false;
                let mut salp1a = tiny();
                let mut calp1a = 1.0;
                let mut salp1b = tiny();
                let mut calp1b = -1.0;
                let mut converged = false;
                let mut last: Lambda;
                loop {
                    last = self.lambda12(
                        sbet1,
                        cbet1,
                        dn1,
                        sbet2,
                        cbet2,
                        dn2,
                        salp1,
                        calp1,
                        slam12,
                        clam12,
                        numit < MAXIT1,
                    );
                    let v = last.lam12;
                    let threshold = if tripn { 8.0 } else { 1.0 } * TOL0;
                    if tripb || fabs(v) < threshold || fabs(v).is_nan() {
                        converged = true;
                        break;
                    }
                    if numit == MAXIT2 {
                        break;
                    }
                    if v > 0.0 && (numit > MAXIT1 || calp1 / salp1 > calp1b / salp1b) {
                        salp1b = salp1;
                        calp1b = calp1;
                    } else if v < 0.0 && (numit > MAXIT1 || calp1 / salp1 < calp1a / salp1a) {
                        salp1a = salp1;
                        calp1a = calp1;
                    }
                    numit += 1;
                    if numit < MAXIT1 && last.dlam12 > 0.0 {
                        let dalp1 = -v / last.dlam12;
                        if fabs(dalp1) < core::f64::consts::PI {
                            let sdalp1 = sin(dalp1);
                            let cdalp1 = cos(dalp1);
                            let nsalp1 = salp1 * cdalp1 + calp1 * sdalp1;
                            if nsalp1 > 0.0 {
                                calp1 = calp1 * cdalp1 - salp1 * sdalp1;
                                salp1 = nsalp1;
                                (salp1, calp1) = norm2(salp1, calp1);
                                tripn = fabs(v) <= 16.0 * TOL0;
                                continue;
                            }
                        }
                    }
                    // Bisect the bracket when Newton's step is unusable.
                    salp1 = (salp1a + salp1b) / 2.0;
                    calp1 = (calp1a + calp1b) / 2.0;
                    (salp1, calp1) = norm2(salp1, calp1);
                    tripn = false;
                    tripb = fabs(salp1a - salp1) + (calp1a - calp1) < TOL0
                        || fabs(salp1 - salp1b) + (calp1 - calp1b) < TOL0;
                }
                if !converged {
                    return Err(GeodesicError::ConvergenceFailure);
                }
                let (s12, _, _) = self.lengths(
                    last.eps, last.sig12, last.ssig1, last.csig1, dn1, last.ssig2, last.csig2, dn2,
                );
                s12x = s12 * self.b;
                salp2 = last.salp2;
                calp2 = last.calp2;
                let (sdomg, cdomg) = (sin(last.domg12), cos(last.domg12));
                omg = Some((slam12 * cdomg - clam12 * sdomg, clam12 * cdomg + slam12 * sdomg));
            }
        }

        // The area term, before the swaps are undone (GeographicLib's `S12`).
        let salp0 = salp1 * cbet1;
        let calp0 = hypot(calp1, salp1 * sbet1);
        let mut s12area = 0.0;
        if calp0 != 0.0 && salp0 != 0.0 {
            let (ssig1, csig1) = norm2(sbet1, calp1 * cbet1);
            let (ssig2, csig2) = norm2(sbet2, calp2 * cbet2);
            let k2 = sq(calp0) * self.ep2;
            let eps = k2 / (2.0 * (1.0 + sqrt(1.0 + k2)) + k2);
            let a4 = sq(self.a) * calp0 * salp0 * self.e2;
            let c4a = self.c4f(eps);
            let b41 = sin_cos_series(false, ssig1, csig1, &c4a);
            let b42 = sin_cos_series(false, ssig2, csig2, &c4a);
            s12area = a4 * (b42 - b41);
        }
        if !meridian {
            let (somg12, comg12) = omg.unwrap_or((0.0, 1.0));
            // The reference's literal threshold, kept as written so results match it exactly.
            #[allow(clippy::approx_constant)]
            let alp12 = if comg12 > -0.7071 && sbet2 - sbet1 < 1.75 {
                let domg12 = 1.0 + comg12;
                let dbet1 = 1.0 + cbet1;
                let dbet2 = 1.0 + cbet2;
                2.0 * atan2(
                    somg12 * (sbet1 * dbet2 + sbet2 * dbet1),
                    domg12 * (sbet1 * sbet2 + dbet1 * dbet2),
                )
            } else {
                let mut salp12 = salp2 * calp1 - calp2 * salp1;
                let mut calp12 = calp2 * calp1 + salp2 * salp1;
                if salp12 == 0.0 && calp12 < 0.0 {
                    salp12 = tiny() * calp1;
                    calp12 = -1.0;
                }
                atan2(salp12, calp12)
            };
            s12area += self.c2 * alp12;
            s12area *= swapp * lonsign * latsign;
        } else {
            // Along a meridian the area is the sphere's term alone.
            let alp12 = {
                let mut salp12 = salp2 * calp1 - calp2 * salp1;
                let mut calp12 = calp2 * calp1 + salp2 * salp1;
                if salp12 == 0.0 && calp12 < 0.0 {
                    salp12 = tiny() * calp1;
                    calp12 = -1.0;
                }
                atan2(salp12, calp12)
            };
            s12area += self.c2 * alp12;
            s12area *= swapp * lonsign * latsign;
        }

        // Undo the swaps and signs applied to the input.
        let s12 = 0.0 + s12x;
        if swapp < 0.0 {
            core::mem::swap(&mut salp2, &mut salp1);
            core::mem::swap(&mut calp2, &mut calp1);
        }
        salp1 *= swapp * lonsign;
        calp1 *= swapp * latsign;
        salp2 *= swapp * lonsign;
        calp2 *= swapp * latsign;
        Ok(InverseSolution { distance_m: s12, area_m2: 0.0 + s12area, salp1, calp1, salp2, calp2 })
    }

    /// The direct problem: the destination `(lat2, lon2)` and the azimuth there `azi2`, for
    /// a geodesic of length `s12` leaving `(lat1, lon1)` at azimuth `azi1`.
    fn direct(&self, lat1: f64, lon1: f64, azi1: f64, s12: f64) -> (f64, f64, f64) {
        let (salp1, calp1) = sincosd(ang_round(azi1));
        let (mut sbet1, mut cbet1) = sincosd(ang_round(lat1));
        sbet1 *= self.f1;
        (sbet1, cbet1) = norm2(sbet1, cbet1);
        cbet1 = fmax(tiny(), cbet1);
        let salp0 = salp1 * cbet1;
        let calp0 = hypot(calp1, salp1 * sbet1);
        let mut ssig1 = sbet1;
        let somg1 = salp0 * sbet1;
        let mut csig1 = if sbet1 != 0.0 || calp1 != 0.0 { cbet1 * calp1 } else { 1.0 };
        let comg1 = csig1;
        (ssig1, csig1) = norm2(ssig1, csig1);
        let k2 = sq(calp0) * self.ep2;
        let eps = k2 / (2.0 * (1.0 + sqrt(1.0 + k2)) + k2);
        let a1m1 = a1m1f(eps);
        let c1a = c1_series(eps);
        let c1pa = c1p_series(eps);
        let b11 = sin_cos_series(true, ssig1, csig1, &c1a);
        let (s0, c0) = (sin(b11), cos(b11));
        let stau1 = ssig1 * c0 + csig1 * s0;
        let ctau1 = csig1 * c0 - ssig1 * s0;
        let c3a = self.c3f(eps);
        let a3c = -self.f * salp0 * self.a3f(eps);
        let b31 = sin_cos_series(true, ssig1, csig1, &c3a);

        // Solve for sigma from the distance (Karney's series inversion).
        let tau12 = s12 / (self.b * (1.0 + a1m1));
        let (st, ct) = (sin(tau12), cos(tau12));
        let b12 = -sin_cos_series(true, stau1 * ct + ctau1 * st, ctau1 * ct - stau1 * st, &c1pa);
        let mut sig12 = tau12 - (b12 - b11);
        let mut ssig12 = sin(sig12);
        let mut csig12 = cos(sig12);
        if fabs(self.f) > 0.01 {
            let ssig2 = ssig1 * csig12 + csig1 * ssig12;
            let csig2 = csig1 * csig12 - ssig1 * ssig12;
            let b12 = sin_cos_series(true, ssig2, csig2, &c1a);
            let serr = (1.0 + a1m1) * (sig12 + (b12 - b11)) - s12 / self.b;
            sig12 -= serr / sqrt(1.0 + k2 * sq(ssig2));
            ssig12 = sin(sig12);
            csig12 = cos(sig12);
        }
        let ssig2 = ssig1 * csig12 + csig1 * ssig12;
        let mut csig2 = csig1 * csig12 - ssig1 * ssig12;
        let sbet2 = calp0 * ssig2;
        let mut cbet2 = hypot(salp0, calp0 * csig2);
        if cbet2 == 0.0 {
            cbet2 = tiny();
            csig2 = tiny();
        }
        let salp2 = salp0;
        let calp2 = calp0 * csig2;
        let somg2 = salp0 * ssig2;
        let comg2 = csig2;
        let omg12 = atan2(somg2 * comg1 - comg2 * somg1, comg2 * comg1 + somg2 * somg1);
        let lam12 = omg12 + a3c * (sig12 + (sin_cos_series(true, ssig2, csig2, &c3a) - b31));
        let lon12 = lam12.to_degrees();
        let lon2 = ang_normalize(ang_normalize(lon1) + ang_normalize(lon12));
        let lat2 = atan2d(sbet2, self.f1 * cbet2);
        let azi2 = atan2d(salp2, calp2);
        (lat2, lon2, azi2)
    }
}

fn fmax(a: f64, b: f64) -> f64 {
    if a >= b || b.is_nan() {
        a
    } else {
        b
    }
}

fn fmin(a: f64, b: f64) -> f64 {
    if a <= b || b.is_nan() {
        a
    } else {
        b
    }
}

/// `A1 - 1` at `eps`.
fn a1m1f(eps: f64) -> f64 {
    let m = ORDER / 2;
    let t = polyval(m, &A1M1_COEFF, 0, sq(eps)) / A1M1_COEFF[m + 1];
    (t + eps) / (1.0 - eps)
}

/// `A2 - 1` at `eps`.
fn a2m1f(eps: f64) -> f64 {
    let m = ORDER / 2;
    let t = polyval(m, &A2M1_COEFF, 0, sq(eps)) / A2M1_COEFF[m + 1];
    (t - eps) / (1.0 + eps)
}

/// `C1[l]` for `l = 0..=6` at `eps` (`c[0]` is unused).
fn c1_series(eps: f64) -> [f64; ORDER + 1] {
    c_series(&C1_COEFF, eps)
}

fn c1p_series(eps: f64) -> [f64; ORDER + 1] {
    c_series(&C1P_COEFF, eps)
}

fn c2_series(eps: f64) -> [f64; ORDER + 1] {
    c_series(&C2_COEFF, eps)
}

/// The `C1`, `C1'` and `C2` families share one layout: `c[l] = eps^l P_l(eps^2)`.
fn c_series(coeff: &[f64], eps: f64) -> [f64; ORDER + 1] {
    let mut c = [0.0; ORDER + 1];
    let eps2 = sq(eps);
    let mut d = eps;
    let mut o = 0;
    for (l, slot) in c.iter_mut().enumerate().skip(1) {
        let m = (ORDER - l) / 2;
        *slot = d * polyval(m, coeff, o, eps2) / coeff[o + m + 1];
        o += m + 2;
        d *= eps;
    }
    c
}

/// The `A3` polynomial coefficients in `eps`, one per power.
fn a3_coeffs(n: f64) -> [f64; ORDER] {
    let mut out = [0.0; ORDER];
    let mut o = 0;
    for (k, j) in (0..ORDER).rev().enumerate() {
        let m = (ORDER - j - 1).min(j);
        out[k] = polyval(m, &A3_COEFF, o, n) / A3_COEFF[o + m + 1];
        o += m + 2;
    }
    out
}

/// The `C4` polynomial coefficients in `eps`, for `l = 0..ORDER`.
fn c4_coeffs(n: f64) -> [f64; 21] {
    let mut out = [0.0; 21];
    let mut o = 0;
    for (k, j) in (0..ORDER).flat_map(|l| (l..ORDER).rev()).enumerate() {
        let m = ORDER - j - 1;
        out[k] = polyval(m, &C4_COEFF, o, n) / C4_COEFF[o + m + 1];
        o += m + 2;
    }
    out
}

/// The `C3` polynomial coefficients in `eps`, for `l = 1..ORDER`.
fn c3_coeffs(n: f64) -> [f64; 15] {
    let mut out = [0.0; 15];
    let mut o = 0;
    let mut k = 0;
    for l in 1..ORDER {
        for j in (l..ORDER).rev() {
            let m = (ORDER - j - 1).min(j);
            out[k] = polyval(m, &C3_COEFF, o, n) / C3_COEFF[o + m + 1];
            k += 1;
            o += m + 2;
        }
    }
    out
}

/// Normalises a bearing in degrees to `[0, 360)`.
fn normalize_bearing_deg(deg: f64) -> f64 {
    ((deg % 360.0) + 360.0) % 360.0
}

fn valid_point(p: GeoPoint) -> bool {
    p.lat_deg.is_finite() && p.lon_deg.is_finite() && (-90.0..=90.0).contains(&p.lat_deg)
}

/// Solves the geodesic inverse problem: given two points, finds the distance and
/// initial/final bearings along the shortest path (geodesic) between them on `ellipsoid`.
///
/// # Errors
/// Returns [`GeodesicError::InvalidInput`] if a coordinate is non-finite or a latitude is
/// outside `[-90, 90]`, or the ellipsoid is degenerate. Returns
/// [`GeodesicError::ConvergenceFailure`] only if the internal iteration is exhausted, which
/// does not happen for valid points.
pub fn inverse(
    ellipsoid: &Ellipsoid,
    p1: GeoPoint,
    p2: GeoPoint,
) -> Result<InverseResult, GeodesicError> {
    if !valid_point(p1) || !valid_point(p2) {
        return Err(GeodesicError::InvalidInput);
    }
    let karney = Karney::new(ellipsoid)?;
    let sol = karney.inverse(p1.lat_deg, p1.lon_deg, p2.lat_deg, p2.lon_deg)?;
    Ok(InverseResult {
        distance_m: sol.distance_m,
        initial_bearing_deg: normalize_bearing_deg(atan2d(sol.salp1, sol.calp1)),
        final_bearing_deg: normalize_bearing_deg(atan2d(sol.salp2, sol.calp2)),
    })
}

/// The perimeter and enclosed area of a polygon on the ellipsoid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PolygonMeasure {
    /// The length of the boundary, including the closing edge, in meters.
    pub perimeter_m: f64,
    /// The enclosed area, in square meters. Counter-clockwise traversal (seen from above)
    /// gives a positive area; a clockwise polygon gives a negative one. A polygon larger than
    /// a hemisphere is reported as the area of its complement, as GeographicLib does.
    pub area_m2: f64,
}

/// The longitude-crossing count of one edge, as GeographicLib's `PolygonArea` counts it.
fn transit(lon1: f64, lon2: f64) -> i32 {
    let (lon12, _) = ang_diff(lon1, lon2);
    let lon1 = ang_normalize(lon1);
    let lon2 = ang_normalize(lon2);
    if lon12 > 0.0 && ((lon1 < 0.0 && 0.0 <= lon2) || (lon1 > 0.0 && lon2 == 0.0)) {
        1
    } else if lon12 < 0.0 && lon2 < 0.0 && 0.0 <= lon1 {
        -1
    } else {
        0
    }
}

/// Measures a polygon given by its vertices in order, as `(latitude, longitude)` degrees.
/// The polygon closes from the last vertex back to the first.
///
/// This follows GeographicLib's `PolygonArea`: the per-edge area terms are summed, and the
/// total is reduced modulo the sphere's area, with the wrap-around counted from the
/// antimeridian crossings.
///
/// # Errors
/// Returns [`GeodesicError::InvalidInput`] for a non-finite coordinate or a latitude outside
/// `[-90, 90]`, and [`GeodesicError::ConvergenceFailure`] if an edge fails to solve.
pub fn polygon_measure(
    ellipsoid: &Ellipsoid,
    vertices: &[GeoPoint],
) -> Result<PolygonMeasure, GeodesicError> {
    if !vertices.iter().all(|&p| valid_point(p)) {
        return Err(GeodesicError::InvalidInput);
    }
    if vertices.len() < 2 {
        return Ok(PolygonMeasure { perimeter_m: 0.0, area_m2: 0.0 });
    }
    let karney = Karney::new(ellipsoid)?;
    let area0 = 4.0 * core::f64::consts::PI * karney.c2;
    let first = vertices[0];
    let mut perimeter = 0.0;
    let mut area_sum = KahanSum::default();
    let mut crossings = 0i32;
    let edges = vertices.iter().zip(vertices.iter().skip(1).chain(core::iter::once(&first)));
    for (from, to) in edges {
        let sol = karney.inverse(from.lat_deg, from.lon_deg, to.lat_deg, to.lon_deg)?;
        perimeter += sol.distance_m;
        area_sum.add(sol.area_m2);
        crossings += transit(from.lon_deg, to.lon_deg);
    }
    Ok(PolygonMeasure {
        perimeter_m: perimeter,
        area_m2: area_reduce(area_sum.value(), area0, crossings),
    })
}

/// A compensated sum, so that many small edge terms add accurately.
#[derive(Default)]
struct KahanSum {
    sum: f64,
    compensation: f64,
}

impl KahanSum {
    fn add(&mut self, value: f64) {
        let y = value - self.compensation;
        let t = self.sum + y;
        self.compensation = (t - self.sum) - y;
        self.sum = t;
    }

    fn value(&self) -> f64 {
        self.sum
    }
}

/// GeographicLib's `_areareduceB` for a counter-clockwise, signed result.
fn area_reduce(area_sum: f64, area0: f64, crossings: i32) -> f64 {
    let mut area = remainder(area_sum, area0);
    if crossings & 1 != 0 {
        area += (if area < 0.0 { 1.0 } else { -1.0 }) * area0 / 2.0;
    }
    area = -area;
    if area > area0 / 2.0 {
        area -= area0;
    } else if area <= -area0 / 2.0 {
        area += area0;
    }
    0.0 + area
}

/// Points at equal arc-length steps along the shortest path from `p1` to `p2`, as an iterator.
///
/// The first point is `p1`, the last is `p2`, and there are `steps + 1` points in all. Each
/// point comes from the direct problem, so the spacing is exact along the ellipsoid, not in
/// degrees.
///
/// # Errors
/// Returns the errors of [`inverse`] for the two end points.
pub fn sample(
    ellipsoid: &Ellipsoid,
    p1: GeoPoint,
    p2: GeoPoint,
    steps: u32,
) -> Result<GeodesicSamples, GeodesicError> {
    let solution = inverse(ellipsoid, p1, p2)?;
    Ok(GeodesicSamples {
        karney: Karney::new(ellipsoid)?,
        start: p1,
        end: p2,
        azimuth_deg: solution.initial_bearing_deg,
        distance_m: solution.distance_m,
        steps,
        index: 0,
    })
}

/// The points produced by [`sample`]: an iterator over the geodesic's equal steps.
#[derive(Debug, Clone)]
pub struct GeodesicSamples {
    karney: Karney,
    start: GeoPoint,
    end: GeoPoint,
    azimuth_deg: f64,
    distance_m: f64,
    steps: u32,
    index: u32,
}

impl Iterator for GeodesicSamples {
    type Item = GeoPoint;

    fn next(&mut self) -> Option<GeoPoint> {
        if self.index > self.steps {
            return None;
        }
        let index = self.index;
        self.index += 1;
        if index == 0 {
            return Some(self.start);
        }
        if index == self.steps {
            // The last point is the end point itself, not a solution that rounding can move.
            return Some(self.end);
        }
        let s = self.distance_m * f64::from(index) / f64::from(self.steps);
        let (lat, mut lon, _) =
            self.karney.direct(self.start.lat_deg, self.start.lon_deg, self.azimuth_deg, s);
        if lon >= 180.0 {
            lon -= 360.0;
        }
        Some(GeoPoint::new(lat, lon))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let left = (self.steps + 1).saturating_sub(self.index) as usize;
        (left, Some(left))
    }
}

/// Solves the geodesic direct problem: given a start point, initial bearing, and
/// distance, finds the destination point and the bearing on arrival, on `ellipsoid`.
///
/// The destination longitude is normalised to `[-180, 180)` degrees.
///
/// # Errors
/// Returns [`GeodesicError::InvalidInput`] if any input is NaN or infinite, the start
/// latitude is outside `[-90, 90]`, or the ellipsoid is degenerate.
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
    let karney = Karney::new(ellipsoid)?;
    let (lat2, mut lon2, azi2) =
        karney.direct(start.lat_deg, start.lon_deg, initial_bearing_deg, distance_m);
    if lon2 >= 180.0 {
        lon2 -= 360.0;
    }
    Ok(DirectResult {
        destination: GeoPoint::new(lat2, lon2),
        final_bearing_deg: normalize_bearing_deg(azi2),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::float_ext::Fabs as _;

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
            (result.distance_m - 54_972.271).fabs() < 1e-3,
            "distance_m = {}",
            result.distance_m
        );

        let expected_initial_bearing = dms_to_deg(306.0, 52.0, 5.37);
        let expected_final_bearing = (dms_to_deg(127.0, 10.0, 25.07) + 180.0) % 360.0;
        assert!(
            (result.initial_bearing_deg - expected_initial_bearing).fabs() < 1e-4,
            "initial_bearing_deg = {}",
            result.initial_bearing_deg
        );
        assert!(
            (result.final_bearing_deg - expected_final_bearing).fabs() < 1e-4,
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
        assert!((result.destination.lat_deg - (-37.652_821)).fabs() < 1e-4);
        assert!((result.destination.lon_deg - 143.926_495).fabs() < 1e-4);
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

    /// GeographicLib's values (see `tests/fixtures/geographiclib_geodesic.csv`): 313 inverse
    /// problems, including nearly antipodal pairs and the poles, and 300 direct problems.
    #[test]
    fn matches_geographiclib_reference() {
        let text = include_str!("../tests/fixtures/geographiclib_geodesic.csv");
        let e = &Ellipsoid::WGS84;
        // Bearings and longitudes are compared modulo 360 degrees.
        let angle_diff = |a: f64, b: f64| {
            let d = ((a - b) % 360.0 + 540.0) % 360.0 - 180.0;
            d.fabs()
        };
        let (mut worst_dist, mut worst_bearing, mut worst_pos, mut checked) =
            (0.0_f64, 0.0_f64, 0.0_f64, 0);
        for line in text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
            let f: [&str; 8] = {
                let mut parts = [""; 8];
                for (slot, part) in parts.iter_mut().zip(line.split(';')) {
                    *slot = part;
                }
                parts
            };
            let mut v = [0.0_f64; 7];
            for (slot, s) in v.iter_mut().zip(f[1..].iter()) {
                *slot = s.parse().unwrap();
            }
            match f[0] {
                "I" => {
                    let r = inverse(e, GeoPoint::new(v[0], v[1]), GeoPoint::new(v[2], v[3]))
                        .unwrap_or_else(|err| panic!("inverse {line}: {err:?}"));
                    worst_dist = worst_dist.max((r.distance_m - v[4]).fabs());
                    worst_bearing = worst_bearing
                        .max(angle_diff(r.initial_bearing_deg, v[5]))
                        .max(angle_diff(r.final_bearing_deg, v[6]));
                }
                "D" => {
                    let r = direct(e, GeoPoint::new(v[0], v[1]), v[2], v[3])
                        .unwrap_or_else(|err| panic!("direct {line}: {err:?}"));
                    worst_pos = worst_pos
                        .max((r.destination.lat_deg - v[4]).fabs())
                        .max(angle_diff(r.destination.lon_deg, v[5]));
                    worst_bearing = worst_bearing.max(angle_diff(r.final_bearing_deg, v[6]));
                }
                other => panic!("unknown row type {other}"),
            }
            checked += 1;
        }
        assert!(checked >= 600, "only {checked} rows");
        // Measured worst cases: distance 1.0e-7 m, bearing 6.9e-10 deg, position 1.9e-11 deg.
        assert!(worst_dist < 1e-6, "worst distance error {worst_dist} m");
        assert!(worst_bearing < 1e-8, "worst bearing error {worst_bearing} deg");
        assert!(worst_pos < 1e-9, "worst destination error {worst_pos} deg");
    }

    /// Nearly antipodal points, where Vincenty's iteration did not converge.
    #[test]
    fn nearly_antipodal_pair_converges() {
        let r = inverse(&Ellipsoid::WGS84, GeoPoint::new(0.5, 0.0), GeoPoint::new(-0.5, 179.7))
            .unwrap();
        assert!((r.distance_m - 19_995_624.889_961).fabs() < 1e-3, "{}", r.distance_m);
    }

    /// GeographicLib's `PolygonArea` results (see `tests/fixtures/geographiclib_polygon.csv`):
    /// 225 polygons, including antimeridian crossings, polar polygons, clockwise rings, and
    /// self-intersecting ones.
    #[test]
    fn polygon_area_matches_geographiclib() {
        let text = include_str!("../tests/fixtures/geographiclib_polygon.csv");
        let e = &Ellipsoid::WGS84;
        let (mut worst_perimeter, mut worst_area, mut checked) = (0.0_f64, 0.0_f64, 0);
        let mut worst_row = "";
        for line in text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
            // Fields: tag, vertex count, the vertices as lat/lon pairs, perimeter, area.
            let mut fields = line.split(';');
            let _tag = fields.next();
            let n: usize = fields.next().unwrap().parse().unwrap();
            let mut vertices = [GeoPoint::new(0.0, 0.0); 16];
            for vertex in vertices.iter_mut().take(n) {
                let lat: f64 = fields.next().unwrap().parse().unwrap();
                let lon: f64 = fields.next().unwrap().parse().unwrap();
                *vertex = GeoPoint::new(lat, lon);
            }
            let want_perimeter: f64 = fields.next().unwrap().parse().unwrap();
            let want_area: f64 = fields.next().unwrap().parse().unwrap();
            let got = polygon_measure(e, &vertices[..n]).unwrap();
            worst_perimeter = worst_perimeter.max((got.perimeter_m - want_perimeter).abs());
            // Relative to the area, with a 1 m² floor for the tiny polygons.
            let rel = (got.area_m2 - want_area).abs() / want_area.abs().max(1.0);
            if rel > worst_area {
                worst_row = line;
            }
            worst_area = worst_area.max(rel);
            checked += 1;
        }
        assert!(checked >= 225, "only {checked} polygons");
        assert!(worst_perimeter < 1e-3, "worst perimeter error {worst_perimeter} m");
        // Measured worst case: 7.3e-9, on a small clockwise polygon. Its edge terms are of the
        // order of the sphere's area, so the sum rounds at about 1 m² there.
        assert!(worst_area < 1e-8, "worst relative area error {worst_area} on {worst_row}");
    }

    /// GeographicLib's `InverseLine` positions (see `tests/fixtures/geographiclib_sample.csv`):
    /// 50 geodesics, each sampled at nine equal steps.
    #[test]
    fn sample_matches_geographiclib_line_positions() {
        let text = include_str!("../tests/fixtures/geographiclib_sample.csv");
        let e = &Ellipsoid::WGS84;
        let mut worst = 0.0_f64;
        let mut checked = 0;
        for line in text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
            let mut fields = line.split(';');
            let mut next = || fields.next().unwrap().parse::<f64>().unwrap();
            let p1 = GeoPoint::new(next(), next());
            let p2 = GeoPoint::new(next(), next());
            let steps = next() as u32;
            let samples = sample(e, p1, p2, steps).unwrap();
            let mut count = 0;
            for (got, index) in samples.zip(0..) {
                let want = GeoPoint::new(next(), next());
                let dlon = ((got.lon_deg - want.lon_deg) % 360.0 + 540.0) % 360.0 - 180.0;
                worst = worst.max((got.lat_deg - want.lat_deg).abs()).max(dlon.abs());
                assert!(index <= steps, "too many samples");
                count += 1;
            }
            assert_eq!(count, steps + 1);
            checked += 1;
        }
        assert_eq!(checked, 50);
        // Measured worst case: 3.0e-10 deg.
        assert!(worst < 1e-9, "worst position error {worst} deg");
    }

    /// The sample at each end is the end point itself.
    #[test]
    fn sample_starts_and_ends_at_the_given_points() {
        let p1 = GeoPoint::new(10.0, 20.0);
        let p2 = GeoPoint::new(-30.0, 60.0);
        let points: [GeoPoint; 5] = {
            let mut out = [p1; 5];
            for (slot, point) in out.iter_mut().zip(sample(&Ellipsoid::WGS84, p1, p2, 4).unwrap()) {
                *slot = point;
            }
            out
        };
        assert_eq!(points[0], p1);
        assert_eq!(points[4], p2);
        assert_eq!(sample(&Ellipsoid::WGS84, p1, p2, 0).unwrap().count(), 1);
    }
}
