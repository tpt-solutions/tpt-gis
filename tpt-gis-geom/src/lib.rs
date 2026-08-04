//! `tpt-gis-geom`: vector geometry primitives and topological predicates.
//!
//! `no_std` by default. Geometries borrow their point data (`&[Point]`) rather than
//! owning it, so they can be built over `const`/`static` data with zero heap
//! allocation — the requirement driving the drone geofence MVP (see `todo.md`).
//!
//! Boolean set operations (`buffer`, `union`, `difference`) are not yet implemented —
//! they require a general polygon-clipping algorithm (e.g. Weiler-Atherton or
//! Martinez-Rueda) that is tracked as a follow-up rather than attempted here.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

pub mod bbox;
pub mod distance;
pub mod line_string;
pub mod point;
pub mod polygon;
pub mod predicates;

pub use bbox::Rect;
pub use line_string::LineString;
pub use point::Point;
pub use polygon::Polygon;
