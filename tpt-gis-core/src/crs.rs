//! Coordinate Reference System (CRS) definitions.

use crate::projection::albers::Albers;
use crate::projection::lcc::LambertConformal;
use crate::projection::utm::{self, Zone};
use crate::projection::web_mercator;
use crate::{Ellipsoid, GeoPoint, PlanarPoint};

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
        }
    }
}

impl Crs {
    /// Converts a coordinate in this CRS to geographic WGS84 degrees.
    ///
    /// For [`Crs::Wgs84`] the planar `x` is longitude and `y` is latitude, so this is the
    /// identity up to that naming. GRS80-based CRSs ([`Crs::Lambert93`], [`Crs::ConusAlbers`])
    /// are treated as WGS84: no datum shift is applied.
    #[must_use]
    pub fn to_geographic(&self, p: PlanarPoint) -> GeoPoint {
        match self {
            Crs::Wgs84 => GeoPoint::new(p.y, p.x),
            Crs::WebMercator => web_mercator::inverse(p),
            Crs::Utm(zone) => utm::inverse(&Ellipsoid::WGS84, p, *zone),
            Crs::Lambert93 => LambertConformal::lambert93().inverse(p),
            Crs::ConusAlbers => Albers::conus_albers().inverse(p),
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
        }
    }
}

/// Transforms a coordinate from one supported CRS to another, through geographic WGS84.
///
/// The two CRSs must both be in this module's set; the transform is exact up to the
/// accuracy of the projections, and the GRS80 CRSs are treated as WGS84 (no datum shift).
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
