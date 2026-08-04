//! The point primitive: a planar (x, y) coordinate.

/// A point in a planar coordinate space (an OGC Simple Features `Point`).
///
/// Unitless — the caller's choice of CRS determines whether `x`/`y` mean
/// longitude/latitude, projected meters, or something else entirely.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    /// X coordinate.
    pub x: f64,
    /// Y coordinate.
    pub y: f64,
}

impl Point {
    /// Constructs a point.
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}
