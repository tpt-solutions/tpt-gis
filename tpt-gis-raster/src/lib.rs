//! `tpt-gis-raster`: grid data, map algebra, resampling, and Cloud Optimized
//! GeoTIFF (COG) parsing.
//!
//! The crate is built from four layers, each usable on its own:
//!
//! * [`grid`] — [`Grid<T>`], a dense row-major array of cells of any
//!   [`CellType`] (`u8`, `u16`, `i16`, `u32`, `i32`, `f32`, `f64`).
//! * [`band`] — [`Band<T>`] (alias [`Raster<T>`]), a `Grid` plus georeferencing:
//!   a [`GeoTransform`] (origin + pixel size), an EPSG code, and a nodata value.
//!   [`AnyBand`] is the dynamically-typed form a file reader returns.
//! * [`algebra`] / [`resample`] — nodata-aware map algebra (local and focal) and
//!   nearest-neighbour / bilinear resampling, generic over the cell type.
//! * [`geotiff`] / [`cog`] — a from-scratch (Geo)TIFF reader (classic and
//!   BigTIFF, strips and tiles, uncompressed / DEFLATE / LZW / PackBits, with
//!   horizontal and floating-point predictors) and a COG layout validator that
//!   exposes the overview pyramid.
//!
//! Nodata is the thread running through all of it: every operation that can see
//! a nodata cell propagates it rather than silently treating the sentinel value
//! (typically `-9999` or `NaN`) as real data.
//!
//! # Features
//!
//! * `std` (default) — enables [`geotiff`] and [`cog`]. The grid, map algebra,
//!   and resampling layers are `no_std` + `alloc` and stay available without it,
//!   so a grid can be manipulated on an embedded target; only the file readers
//!   need `std` (DEFLATE decoding is exposed as `std::io` streams by `flate2`).
//! * `http` (off by default) — the async [`http`] module: a Range-request
//!   reader that fetches only the header and the tiles actually asked for, so a
//!   remote COG can be read without downloading it. Implies `std` and pulls in
//!   `reqwest` (rustls, no OpenSSL FFI).
//!
//! ```
//! use tpt_gis_raster::{algebra, Band, GeoTransform, Grid};
//!
//! let grid = Grid::new(2, 2, vec![1i16, 2, -9999, 4]).unwrap();
//! let band = Band::new(grid, GeoTransform::new(0.0, 10.0, 1.0, 1.0)).with_nodata(-9999);
//!
//! let doubled = algebra::add(&band, &band).unwrap();
//! assert_eq!(doubled.get(0, 0), Some(2));
//! // Nodata in, nodata out — not -19998.
//! assert_eq!(doubled.get(0, 1), Some(-9999));
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

extern crate alloc;

pub mod algebra;
pub mod band;
pub mod cell;
pub mod error;
pub mod grid;
pub mod resample;

#[cfg(feature = "std")]
pub mod cog;
#[cfg(feature = "std")]
pub mod geotiff;
#[cfg(feature = "http")]
pub mod http;

pub use band::{AnyBand, Band, GeoTransform, Raster};
pub use cell::{CellKind, CellType};
pub use error::RasterError;
pub use grid::Grid;
pub use resample::{Resampling, sample_average, sample_bilinear, sample_nearest};
