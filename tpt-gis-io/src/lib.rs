//! `tpt-gis-io`: parsers and writers for GeoJSON, WKB/WKT, Shapefile, and GeoPackage.
//!
//! Unlike `tpt-gis-core`/`tpt-gis-geom`, this crate is not `no_std` — parsing
//! variable-sized file/stream data inherently needs heap allocation, so there is no
//! reason to fight that here. See [`geometry`] for the shared owned geometry
//! representation every format reader/writer in this crate produces and consumes.
//!
//! GeoPackage (`.gpkg`) is, per the OGC spec, a SQLite database file. This crate
//! reads and writes it with **no C FFI** and **no external SQLite engine** — the
//! minimal SQLite file-format logic lives in the [`geopackage`] module, which is
//! pure-Rust and zero-dependency. The writer is intentionally limited to
//! single-leaf-page tables (no spatial index, no overflow pages); see that module's
//! docs for the exact scope.
#![warn(missing_docs)]

pub mod geojson;
pub mod geometry;
pub mod geopackage;
pub mod shapefile;
pub mod wkb;
pub mod wkt;

pub use geometry::{Geometry, Point, Polygon};
