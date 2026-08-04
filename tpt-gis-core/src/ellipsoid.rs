//! Reference ellipsoid models used to approximate the Earth's shape.

/// A biaxial reference ellipsoid, defined by its semi-major axis and flattening.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ellipsoid {
    /// Semi-major axis (equatorial radius), in meters.
    pub semi_major_axis: f64,
    /// Flattening, `f = (a - b) / a`.
    pub flattening: f64,
}

impl Ellipsoid {
    /// Constructs an ellipsoid from its semi-major axis and inverse flattening (`1/f`).
    #[must_use]
    pub const fn from_inverse_flattening(semi_major_axis: f64, inverse_flattening: f64) -> Self {
        Self { semi_major_axis, flattening: 1.0 / inverse_flattening }
    }

    /// Semi-minor axis (polar radius), `b = a * (1 - f)`.
    #[must_use]
    pub fn semi_minor_axis(&self) -> f64 {
        self.semi_major_axis * (1.0 - self.flattening)
    }

    /// First eccentricity squared, `e^2 = f * (2 - f)`.
    #[must_use]
    pub fn eccentricity_squared(&self) -> f64 {
        self.flattening * (2.0 - self.flattening)
    }

    /// Second eccentricity squared, `e'^2 = e^2 / (1 - e^2)`.
    #[must_use]
    pub fn second_eccentricity_squared(&self) -> f64 {
        let e2 = self.eccentricity_squared();
        e2 / (1.0 - e2)
    }

    /// WGS84 ellipsoid — the default reference for GPS and most modern web mapping.
    pub const WGS84: Self = Self::from_inverse_flattening(6_378_137.0, 298.257_223_563);

    /// GRS80 ellipsoid — used by most national geodetic datums (e.g. NAD83, GDA94/GDA2020).
    /// Numerically near-identical to WGS84 at the sub-millimeter level.
    pub const GRS80: Self = Self::from_inverse_flattening(6_378_137.0, 298.257_222_101);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wgs84_semi_minor_axis() {
        let b = Ellipsoid::WGS84.semi_minor_axis();
        assert!((b - 6_356_752.314_245).abs() < 1e-3);
    }
}
