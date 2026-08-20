//! One-line ergonomic imports for the most common `tpt-gis` types.
//!
//! Pulls in the primary geometry, geodesy, and raster types (gated by the same
//! feature flags as the rest of the facade) so a consumer can write:
//!
//! ```
//! use tpt_gis::prelude::*;
//!
//! let london = GeoPoint::new(51.5074, -0.1278);
//! let paris = GeoPoint::new(48.8566, 2.3522);
//! let km = geodesic::inverse(&Ellipsoid::WGS84, london, paris).unwrap().distance_m / 1000.0;
//! let _ = km;
//! ```

#![cfg_attr(not(feature = "std"), allow(unused_imports))]

#[cfg(feature = "core")]
pub use tpt_gis_core::{
    crs::Crs,
    datum::{geocentric_to_geodetic, geodetic_to_geocentric, transform_geocentric, HelmertParams},
    ellipsoid::Ellipsoid,
    epsg, geodesic, GeoPoint,
};

#[cfg(feature = "geom")]
pub use tpt_gis_geom::{distance, LineString, Point, Polygon, Rect};

#[cfg(feature = "io")]
pub use tpt_gis_io::geometry::Geometry;

#[cfg(feature = "index")]
pub use tpt_gis_index::RTree;

#[cfg(feature = "raster")]
pub use tpt_gis_raster::{
    AnyBand, Band, CellKind, CellType, GeoTransform, Grid, Raster, RasterError,
};
