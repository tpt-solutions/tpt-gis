//! Coordinate Reference System (CRS) definitions.

use crate::datum::{self, Geocentric, HelmertParams};
use crate::projection::albers::Albers;
use crate::projection::lcc::LambertConformal;
use crate::projection::transverse_mercator;
use crate::projection::utm::{self, Zone};
use crate::projection::web_mercator;
use crate::{Ellipsoid, GeoPoint, PlanarPoint};

/// Ordnance Survey's Helmert transformation from OSGB36 (Airy 1830) geodetic coordinates to
/// WGS84: translations in metres, rotations in arc-seconds, scale in parts per million. These
/// are the published values for "OSGB36 to WGS 84 (1)" (EPSG:1314). Accuracy is about 5 m across
/// Great Britain, because the OSTN15 grid, which is more accurate, is not used.
const OSGB36_TO_WGS84: HelmertParams = {
    const ARCSEC: f64 = core::f64::consts::PI / (180.0 * 3600.0);
    HelmertParams {
        tx: 446.448,
        ty: -125.157,
        tz: 542.060,
        rx: 0.1502 * ARCSEC,
        ry: 0.2470 * ARCSEC,
        rz: 0.8421 * ARCSEC,
        scale: -20.4894e-6,
    }
};

/// A coordinate reference system supported by `tpt-gis-core`.
///
/// This is a deliberately small, curated set for the project's early phases (see
/// `todo.md`) rather than a full EPSG database — [`crate::epsg`] documents how this
/// will grow.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Crs {
    /// Geographic WGS84 (EPSG:4326): longitude/latitude in decimal degrees.
    Wgs84,
    /// Web Mercator (EPSG:3857): the spherical projection used by web map tiles.
    WebMercator,
    /// Universal Transverse Mercator, in a specific zone (EPSG:326xx / 327xx).
    Utm(Zone),
    /// RGF93 / Lambert-93, the French national grid (EPSG:2154).
    Lambert93,
    /// NAD83 / Conus Albers, the contiguous-US equal-area grid (EPSG:5070).
    ConusAlbers,
    /// OSGB36 geographic (EPSG:4277): longitude/latitude on the Airy 1830 ellipsoid. Converted to
    /// WGS84 with the Ordnance Survey's Helmert transform (about 5 m accuracy).
    Osgb36,
    /// The British National Grid (EPSG:27700): a Transverse Mercator on OSGB36, with the same
    /// Helmert accuracy as [`Crs::Osgb36`].
    BritishNationalGrid,
}

impl Crs {
    /// The EPSG code identifying this CRS, if it has one in `tpt-gis-core`'s registry.
    #[must_use]
    pub fn epsg_code(&self) -> u32 {
        match self {
            Crs::Wgs84 => 4326,
            Crs::WebMercator => 3857,
            Crs::Utm(zone) => zone.epsg_code(),
            Crs::Lambert93 => 2154,
            Crs::ConusAlbers => 5070,
            Crs::Osgb36 => 4277,
            Crs::BritishNationalGrid => 27700,
        }
    }
}

/// Converts OSGB36 geodetic coordinates to WGS84 with the Helmert transform. The height is zero
/// and is discarded.
fn osgb36_to_wgs84(g: GeoPoint) -> GeoPoint {
    let geocentric = datum::geodetic_to_geocentric(&Ellipsoid::AIRY_1830, g, 0.0);
    let shifted = datum::transform_geocentric(&OSGB36_TO_WGS84, geocentric);
    datum::geocentric_to_geodetic(&Ellipsoid::WGS84, shifted).0
}

/// Converts WGS84 geodetic coordinates to OSGB36 with the inverse Helmert transform.
fn wgs84_to_osgb36(g: GeoPoint) -> GeoPoint {
    let geocentric: Geocentric = datum::geodetic_to_geocentric(&Ellipsoid::WGS84, g, 0.0);
    let shifted = datum::inverse_transform_geocentric(&OSGB36_TO_WGS84, geocentric);
    datum::geocentric_to_geodetic(&Ellipsoid::AIRY_1830, shifted).0
}

impl Crs {
    /// Converts a coordinate in this CRS to geographic WGS84 degrees.
    ///
    /// For [`Crs::Wgs84`] the planar `x` is longitude and `y` is latitude, so this is the
    /// identity up to that naming. GRS80-based CRSs ([`Crs::Lambert93`], [`Crs::ConusAlbers`])
    /// are treated as WGS84: no datum shift is applied. The OSGB36 CRSs ([`Crs::Osgb36`],
    /// [`Crs::BritishNationalGrid`]) are shifted with the Ordnance Survey's Helmert transform.
    #[must_use]
    pub fn to_geographic(&self, p: PlanarPoint) -> GeoPoint {
        match self {
            Crs::Wgs84 => GeoPoint::new(p.y, p.x),
            Crs::WebMercator => web_mercator::inverse(p),
            Crs::Utm(zone) => utm::inverse(&Ellipsoid::WGS84, p, *zone),
            Crs::Lambert93 => LambertConformal::lambert93().inverse(p),
            Crs::ConusAlbers => Albers::conus_albers().inverse(p),
            Crs::Osgb36 => osgb36_to_wgs84(GeoPoint::new(p.y, p.x)),
            Crs::BritishNationalGrid => {
                osgb36_to_wgs84(transverse_mercator::british_national_grid().inverse(p))
            }
        }
    }

    /// Converts a geographic WGS84 coordinate to this CRS. The inverse of
    /// [`Crs::to_geographic`].
    #[must_use]
    pub fn from_geographic(&self, g: GeoPoint) -> PlanarPoint {
        match self {
            Crs::Wgs84 => PlanarPoint::new(g.lon_deg, g.lat_deg),
            Crs::WebMercator => web_mercator::forward(g),
            Crs::Utm(zone) => utm::forward(&Ellipsoid::WGS84, g, *zone),
            Crs::Lambert93 => LambertConformal::lambert93().forward(g),
            Crs::ConusAlbers => Albers::conus_albers().forward(g),
            Crs::Osgb36 => {
                let o = wgs84_to_osgb36(g);
                PlanarPoint::new(o.lon_deg, o.lat_deg)
            }
            Crs::BritishNationalGrid => {
                transverse_mercator::british_national_grid().forward(wgs84_to_osgb36(g))
            }
        }
    }
}

/// Transforms a coordinate from one supported CRS to another, through geographic WGS84.
///
/// The two CRSs must both be in this module's set; the transform is exact up to the
/// accuracy of the projections, and the GRS80 CRSs are treated as WGS84 (no datum shift). The
/// OSGB36 CRSs use the Helmert transform, accurate to about 5 m.
///
/// # Examples
///
/// ```
/// use tpt_gis_core::crs::{transform, Crs};
/// use tpt_gis_core::projection::utm::Zone;
/// use tpt_gis_core::PlanarPoint;
///
/// // A point in UTM zone 33N, moved to Lambert-93.
/// let utm = Crs::Utm(Zone::new(33, true));
/// let p = utm.from_geographic(tpt_gis_core::GeoPoint::new(48.8566, 15.0));
/// let lambert = transform(p, utm, Crs::Lambert93);
/// assert!(lambert.x.is_finite() && lambert.y.is_finite());
/// ```
#[must_use]
pub fn transform(p: PlanarPoint, from: Crs, to: Crs) -> PlanarPoint {
    to.from_geographic(from.to_geographic(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::float_ext::Fabs as _;

    #[test]
    fn osgb36_and_british_grid_have_their_epsg_codes() {
        assert_eq!(Crs::Osgb36.epsg_code(), 4277);
        assert_eq!(Crs::BritishNationalGrid.epsg_code(), 27700);
    }

    /// The OSGB36 to WGS84 round trip through the 2D interface is not exact. The ellipsoidal
    /// height is not carried, and at Caister (about 45 m) that leaves a few centimetres. The
    /// bound records that; the 3D inverse is checked to floating point in `datum`.
    #[test]
    fn helmert_round_trip_in_2d_is_within_a_few_centimetres() {
        let original = GeoPoint::new(52.657_568_08, 1.717_921_59);
        let back = wgs84_to_osgb36(osgb36_to_wgs84(original));
        let dlat_m = (back.lat_deg - original.lat_deg).to_radians() * 6_378_137.0;
        let dlon_m = (back.lon_deg - original.lon_deg).to_radians()
            * 6_378_137.0
            * original.lat_deg.to_radians().cos();
        let error = dlat_m.hypot(dlon_m);
        assert!(error < 0.05, "round trip error {error} m");
    }

    #[test]
    fn british_grid_round_trips_through_wgs84_within_a_few_centimetres() {
        let crs = Crs::BritishNationalGrid;
        let p = PlanarPoint::new(651_409.903, 313_177.270);
        let back = crs.from_geographic(crs.to_geographic(p));
        assert!((back.x - p.x).fabs() < 0.05, "easting {}", back.x);
        assert!((back.y - p.y).fabs() < 0.05, "northing {}", back.y);
    }
}
