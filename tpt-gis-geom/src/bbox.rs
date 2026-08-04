//! Axis-aligned bounding boxes (envelopes).

use crate::point::Point;

/// An axis-aligned bounding box (envelope).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Minimum (bottom-left) corner.
    pub min: Point,
    /// Maximum (top-right) corner.
    pub max: Point,
}

impl Rect {
    /// Constructs a bounding box from explicit min/max corners.
    #[must_use]
    pub const fn new(min: Point, max: Point) -> Self {
        Self { min, max }
    }

    /// Computes the bounding box enclosing a non-empty slice of points.
    #[must_use]
    pub fn from_points(points: &[Point]) -> Option<Self> {
        let first = points.first()?;
        let mut min = *first;
        let mut max = *first;
        for p in &points[1..] {
            min.x = min.x.min(p.x);
            min.y = min.y.min(p.y);
            max.x = max.x.max(p.x);
            max.y = max.y.max(p.y);
        }
        Some(Self { min, max })
    }

    /// Merges two bounding boxes into the smallest box enclosing both.
    #[must_use]
    pub fn union(&self, other: &Rect) -> Rect {
        Rect {
            min: Point::new(self.min.x.min(other.min.x), self.min.y.min(other.min.y)),
            max: Point::new(self.max.x.max(other.max.x), self.max.y.max(other.max.y)),
        }
    }

    /// Returns `true` if this box and `other` overlap (including touching at an edge).
    #[must_use]
    pub fn intersects(&self, other: &Rect) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
    }

    /// Returns `true` if `point` lies within this box (inclusive of the boundary).
    #[must_use]
    pub fn contains_point(&self, point: Point) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_points_computes_envelope() {
        let pts = [Point::new(1.0, 5.0), Point::new(-2.0, 3.0), Point::new(4.0, -1.0)];
        let rect = Rect::from_points(&pts).unwrap();
        assert_eq!(rect.min, Point::new(-2.0, -1.0));
        assert_eq!(rect.max, Point::new(4.0, 5.0));
    }

    #[test]
    fn empty_slice_has_no_envelope() {
        assert_eq!(Rect::from_points(&[]), None);
    }

    #[test]
    fn disjoint_boxes_do_not_intersect() {
        let a = Rect::new(Point::new(0.0, 0.0), Point::new(1.0, 1.0));
        let b = Rect::new(Point::new(2.0, 2.0), Point::new(3.0, 3.0));
        assert!(!a.intersects(&b));
    }

    #[test]
    fn touching_boxes_intersect() {
        let a = Rect::new(Point::new(0.0, 0.0), Point::new(1.0, 1.0));
        let b = Rect::new(Point::new(1.0, 0.0), Point::new(2.0, 1.0));
        assert!(a.intersects(&b));
    }
}
