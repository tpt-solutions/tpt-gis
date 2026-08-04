//! `tpt-gis-core`: coordinate reference systems, datum transformations, and geodesic math.
//!
//! `no_std` by default (uses [`libm`] for floating-point transcendental functions so it
//! runs without the Rust standard library). Enable the `std` feature to opt into the
//! platform's native math intrinsics instead.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

pub mod crs;
pub mod datum;
pub mod ellipsoid;
pub mod epsg;
pub mod geodesic;
pub mod projection;

pub use crs::Crs;
pub use ellipsoid::Ellipsoid;

/// A geographic coordinate (angular position on an ellipsoid), in decimal degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeoPoint {
    /// Latitude in decimal degrees, positive north.
    pub lat_deg: f64,
    /// Longitude in decimal degrees, positive east.
    pub lon_deg: f64,
}

impl GeoPoint {
    /// Constructs a geographic coordinate from decimal degrees.
    #[must_use]
    pub const fn new(lat_deg: f64, lon_deg: f64) -> Self {
        Self { lat_deg, lon_deg }
    }

    /// Latitude in radians.
    #[must_use]
    pub fn lat_rad(&self) -> f64 {
        self.lat_deg.to_radians()
    }

    /// Longitude in radians.
    #[must_use]
    pub fn lon_rad(&self) -> f64 {
        self.lon_deg.to_radians()
    }
}

/// A planar (projected) coordinate, in the linear unit of its CRS (typically meters).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlanarPoint {
    /// Easting / X coordinate.
    pub x: f64,
    /// Northing / Y coordinate.
    pub y: f64,
}

impl PlanarPoint {
    /// Constructs a planar coordinate.
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}
