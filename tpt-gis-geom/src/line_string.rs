//! The `LineString` primitive: an ordered sequence of points forming a path.

use crate::bbox::Rect;
use crate::distance::point_to_point;
use crate::point::Point;

/// An ordered sequence of points forming a path (an OGC Simple Features `LineString`).
///
/// Borrows its points rather than owning them, so it can be built over `const`/`static`
/// data with zero heap allocation — a requirement for `no_std` embedded use (see the
/// drone geofence example).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineString<'a> {
    /// The points making up the path, in order.
    pub points: &'a [Point],
}

impl<'a> LineString<'a> {
    /// Wraps a slice of points as a `LineString`.
    #[must_use]
    pub const fn new(points: &'a [Point]) -> Self {
        Self { points }
    }

    /// The bounding box enclosing this line, or `None` if it has no points.
    #[must_use]
    pub fn bbox(&self) -> Option<Rect> {
        Rect::from_points(self.points)
    }

    /// Iterates over the line's segments as `(start, end)` point pairs.
    pub fn segments(&self) -> impl Iterator<Item = (Point, Point)> + 'a {
        self.points.windows(2).map(|w| (w[0], w[1]))
    }

    /// The planar length of the path: the sum of its segment lengths.
    #[must_use]
    pub fn length(&self) -> f64 {
        self.segments().map(|(a, b)| point_to_point(a, b)).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_sums_segments() {
        let pts = [Point::new(0.0, 0.0), Point::new(3.0, 4.0), Point::new(3.0, 0.0)];
        let line = LineString::new(&pts);
        assert!((line.length() - 9.0).abs() < 1e-9); // 5.0 + 4.0
    }

    #[test]
    fn single_point_has_zero_length() {
        let pts = [Point::new(1.0, 1.0)];
        let line = LineString::new(&pts);
        assert_eq!(line.length(), 0.0);
    }
}
