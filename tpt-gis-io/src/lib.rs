//! `tpt-gis-io`: parsers and writers for GeoJSON, WKB/WKT, and Shapefile.
//!
//! Unlike `tpt-gis-core`/`tpt-gis-geom`, this crate is not `no_std` — parsing
//! variable-sized file/stream data inherently needs heap allocation, so there is no
//! reason to fight that here. See [`geometry`] for the shared owned geometry
//! representation every format reader/writer in this crate produces and consumes.
//!
//! GeoPackage read/write is not yet implemented: it's a SQLite-based format, and
//! the obvious pure-Rust path (a from-scratch SQLite-compatible engine) is still
//! experimental in the Rust ecosystem, while the mature option (`rusqlite`) wraps
//! the C SQLite library via FFI — a direct conflict with this project's zero-FFI
//! principle. Revisit once a pure-Rust SQLite engine is production-ready.
#![warn(missing_docs)]

pub mod geojson;
pub mod geometry;
pub mod shapefile;
pub mod wkb;
pub mod wkt;

pub use geometry::{Geometry, Point, Polygon};
