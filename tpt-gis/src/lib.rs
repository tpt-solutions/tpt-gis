//! `tpt-gis` — a pure Rust, `no_std`-capable, planetary-scale GIS engine.
//!
//! This is a convenience facade that re-exports the five engine crates behind
//! feature flags, so downstream users can depend on a single crate:
//!
//! ```
//! # #[cfg(feature = "geom")]
//! # fn example() {
//! use tpt_gis::geom::Point;
//! let p = Point::new(1.0, 2.0);
//! # let _ = p;
//! # }
//! ```
//!
//! ## Feature flags
//!
//! All five engine crates are enabled by default. Each can be toggled
//! individually; `io`/`index` imply `geom`, and `http` implies `raster` plus
//! `tpt-gis-raster`'s async HTTP Range-request reader (which pulls in
//! `reqwest`/`tokio`):
//!
//! - `core` — CRS definitions, datum transforms, geodesic math
//! - `geom` — vector geometry + topological predicates
//! - `io` — GeoJSON/WKB/WKT/Shapefile parsers + writers
//! - `index` — R-Trees, Quadtrees, H3/S2
//! - `raster` — grid data, map algebra, COG/GeoTIFF parsing
//! - `http` — async HTTP Range-request streaming reader for remote COGs
//! - `std` — host `std` math intrinsics for `core`/`geom` (on by default)

#[cfg(feature = "core")]
pub use tpt_gis_core as core;

#[cfg(feature = "geom")]
pub use tpt_gis_geom as geom;

#[cfg(feature = "io")]
pub use tpt_gis_io as io;

#[cfg(feature = "index")]
pub use tpt_gis_index as index;

#[cfg(feature = "raster")]
pub use tpt_gis_raster as raster;
