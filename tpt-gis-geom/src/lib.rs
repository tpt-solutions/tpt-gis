//! `tpt-gis-geom`: vector geometry primitives and topological predicates.
//!
//! `no_std` by default. Geometries borrow their point data (`&[Point]`) rather than
//! owning it, so they can be built over `const`/`static` data with zero heap
//! allocation — the requirement driving the drone geofence MVP (see `todo.md`).
//!
//! Polygon boolean operations (`union`, `intersection`, `difference`,
//! `symmetric_difference`) are in [`clip`], behind the `alloc` feature, because their
//! output must be allocated. They use the Martinez–Rueda–Feito sweep. `buffer` is not
//! implemented yet.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod bbox;
#[cfg(feature = "alloc")]
pub mod buffer;
#[cfg(feature = "alloc")]
pub mod clip;
pub mod distance;
pub mod hull;
pub mod line_string;
pub mod measure;
pub mod multi;
pub mod overlay;
pub mod point;
pub mod polygon;
pub mod predicates;
#[cfg(feature = "alloc")]
pub mod relate;
pub mod simplify;
pub mod validity;

pub use bbox::Rect;
pub use hull::convex_hull;
pub use line_string::LineString;
pub use measure::ring_signed_area;
pub use multi::{GeometryRef, MultiLineString, MultiPoint, MultiPolygon};
pub use overlay::{polygon_within, segment_intersection};
pub use point::Point;
pub use polygon::Polygon;
