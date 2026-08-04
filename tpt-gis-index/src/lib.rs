//! `tpt-gis-index`: spatial indexing — R-Trees, Quadtrees, and H3 hexagonal grid
//! integration.
//!
//! S2 grid integration is not yet implemented: the only pure-Rust S2 crate
//! (`s2`/`rust-s2`) found on crates.io is at v0.1.0 and appears too early-stage to
//! depend on for a project targeting production use — revisit once it (or another
//! pure-Rust S2 implementation) matures. H3 support instead wraps
//! [`h3o`](https://docs.rs/h3o), a mature, actively-maintained, from-scratch pure-Rust
//! reimplementation of Uber's H3 (BSD-3-Clause, zero FFI, zero unsafe) — reimplementing
//! H3 itself from scratch was judged not worth the risk/effort given a solid pure-Rust
//! option already exists.
#![warn(missing_docs)]

pub mod h3;
pub mod quadtree;
pub mod rtree;
pub mod spatial_join;

pub use rtree::RTree;
