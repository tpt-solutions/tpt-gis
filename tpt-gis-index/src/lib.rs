//! `tpt-gis-index`: spatial indexing — R-Trees, Quadtrees, and H3 / S2 hexagonal
//! grid integration.
//!
//! H3 support wraps [`h3o`](https://docs.rs/h3o), a mature, actively-maintained,
//! from-scratch pure-Rust reimplementation of Uber's H3 (BSD-3-Clause, zero FFI,
//! zero unsafe). S2 support is a from-scratch, pure-Rust implementation in the
//! [`s2`] module. It uses S2-style cube-face cell ids, but its ordering is a
//! Z-order (Morton) curve rather than Google's Hilbert curve, and tokens are fixed
//! 16-digit hex. Ids are therefore **not** bit-compatible with Google's S2; treat
//! them as internal to this crate.
#![warn(missing_docs)]

pub mod h3;
pub mod quadtree;
pub mod rtree;
pub mod s2;
pub mod spatial_join;

pub use rtree::RTree;
