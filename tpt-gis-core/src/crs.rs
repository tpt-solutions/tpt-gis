//! Coordinate Reference System (CRS) definitions.

use crate::projection::utm::Zone;

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
}

impl Crs {
    /// The EPSG code identifying this CRS, if it has one in `tpt-gis-core`'s registry.
    #[must_use]
    pub fn epsg_code(&self) -> u32 {
        match self {
            Crs::Wgs84 => 4326,
            Crs::WebMercator => 3857,
            Crs::Utm(zone) => zone.epsg_code(),
        }
    }
}
