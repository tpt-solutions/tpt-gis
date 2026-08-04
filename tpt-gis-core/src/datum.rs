//! Datum transformations: converting coordinates between different reference frames
//! (e.g. a local national datum and WGS84), via geocentric (ECEF) coordinates and a
//! Helmert (Bursa-Wolf) 7-parameter transformation.

use libm::{atan2, cos, sin, sqrt};

use crate::{Ellipsoid, GeoPoint};

/// A geocentric (Earth-Centered, Earth-Fixed / ECEF) Cartesian coordinate, in meters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Geocentric {
    /// X coordinate, in meters.
    pub x: f64,
    /// Y coordinate, in meters.
    pub y: f64,
    /// Z coordinate, in meters.
    pub z: f64,
}

impl Geocentric {
    /// Constructs a geocentric coordinate.
    #[must_use]
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

/// Converts a geodetic coordinate (latitude, longitude, ellipsoidal height) to
/// geocentric (ECEF) Cartesian coordinates.
#[must_use]
pub fn geodetic_to_geocentric(ellipsoid: &Ellipsoid, point: GeoPoint, height_m: f64) -> Geocentric {
    let a = ellipsoid.semi_major_axis;
    let e2 = ellipsoid.eccentricity_squared();

    let phi = point.lat_rad();
    let lambda = point.lon_rad();
    let (sin_phi, cos_phi) = (sin(phi), cos(phi));

    let n = a / sqrt(1.0 - e2 * sin_phi * sin_phi);

    Geocentric {
        x: (n + height_m) * cos_phi * cos(lambda),
        y: (n + height_m) * cos_phi * sin(lambda),
        z: (n * (1.0 - e2) + height_m) * sin_phi,
    }
}

/// Converts geocentric (ECEF) Cartesian coordinates to a geodetic coordinate,
/// returning the point and its ellipsoidal height in meters.
///
/// Uses Bowring's iterative method, which converges to sub-millimeter precision
/// within a handful of iterations for all points outside the immediate vicinity of
/// the Earth's center.
#[must_use]
pub fn geocentric_to_geodetic(ellipsoid: &Ellipsoid, geocentric: Geocentric) -> (GeoPoint, f64) {
    let a = ellipsoid.semi_major_axis;
    let e2 = ellipsoid.eccentricity_squared();

    let p = sqrt(geocentric.x * geocentric.x + geocentric.y * geocentric.y);
    let lambda = atan2(geocentric.y, geocentric.x);

    let mut phi = atan2(geocentric.z, p * (1.0 - e2));
    for _ in 0..10 {
        let sin_phi = sin(phi);
        let n = a / sqrt(1.0 - e2 * sin_phi * sin_phi);
        let h = p / cos(phi) - n;
        let phi_new = atan2(geocentric.z, p * (1.0 - e2 * n / (n + h)));
        if (phi_new - phi).abs() < 1e-13 {
            phi = phi_new;
            break;
        }
        phi = phi_new;
    }

    let sin_phi = sin(phi);
    let n = a / sqrt(1.0 - e2 * sin_phi * sin_phi);
    let height_m = p / cos(phi) - n;

    (GeoPoint::new(phi.to_degrees(), lambda.to_degrees()), height_m)
}

/// Parameters for a 7-parameter Helmert (Bursa-Wolf) datum transformation, applied to
/// geocentric coordinates using the "position vector" rotation convention (EPSG method 9606).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HelmertParams {
    /// Translation along X, in meters.
    pub tx: f64,
    /// Translation along Y, in meters.
    pub ty: f64,
    /// Translation along Z, in meters.
    pub tz: f64,
    /// Rotation about X, in radians.
    pub rx: f64,
    /// Rotation about Y, in radians.
    pub ry: f64,
    /// Rotation about Z, in radians.
    pub rz: f64,
    /// Scale correction, dimensionless (e.g. parts-per-million / 1e6).
    pub scale: f64,
}

impl HelmertParams {
    /// Builds parameters from the units they are conventionally published in:
    /// translations in meters, rotations in arc-seconds, and scale in parts-per-million.
    #[must_use]
    pub fn from_arcsec_ppm(
        tx: f64,
        ty: f64,
        tz: f64,
        rx_arcsec: f64,
        ry_arcsec: f64,
        rz_arcsec: f64,
        scale_ppm: f64,
    ) -> Self {
        const ARCSEC_TO_RAD: f64 = core::f64::consts::PI / (180.0 * 3600.0);
        Self {
            tx,
            ty,
            tz,
            rx: rx_arcsec * ARCSEC_TO_RAD,
            ry: ry_arcsec * ARCSEC_TO_RAD,
            rz: rz_arcsec * ARCSEC_TO_RAD,
            scale: scale_ppm * 1e-6,
        }
    }

    /// The identity transformation (no translation, rotation, or scale change).
    pub const IDENTITY: Self =
        Self { tx: 0.0, ty: 0.0, tz: 0.0, rx: 0.0, ry: 0.0, rz: 0.0, scale: 0.0 };
}

/// Applies a Helmert 7-parameter transformation to a geocentric coordinate, under the
/// small-angle approximation standard to Bursa-Wolf datum transformations.
#[must_use]
pub fn transform_geocentric(params: &HelmertParams, point: Geocentric) -> Geocentric {
    let scale_factor = 1.0 + params.scale;
    Geocentric {
        x: params.tx + scale_factor * (point.x - params.rz * point.y + params.ry * point.z),
        y: params.ty + scale_factor * (params.rz * point.x + point.y - params.rx * point.z),
        z: params.tz + scale_factor * (-params.ry * point.x + params.rx * point.y + point.z),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geodetic_geocentric_round_trip() {
        let original = GeoPoint::new(-37.951_033, 144.424_868);
        let height = 100.0;

        let geocentric = geodetic_to_geocentric(&Ellipsoid::WGS84, original, height);
        let (recovered, recovered_height) = geocentric_to_geodetic(&Ellipsoid::WGS84, geocentric);

        assert!((recovered.lat_deg - original.lat_deg).abs() < 1e-9);
        assert!((recovered.lon_deg - original.lon_deg).abs() < 1e-9);
        assert!((recovered_height - height).abs() < 1e-6);
    }

    #[test]
    fn identity_transform_is_a_no_op() {
        let p = Geocentric::new(1_000_000.0, 2_000_000.0, 3_000_000.0);
        let result = transform_geocentric(&HelmertParams::IDENTITY, p);
        assert_eq!(result, p);
    }
}
