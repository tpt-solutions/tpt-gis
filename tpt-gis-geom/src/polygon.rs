//! The `Polygon` primitive: an exterior ring with zero or more interior (hole) rings.

use crate::bbox::Rect;
use crate::distance;
use crate::point::Point;
use crate::predicates;

/// A polygon (an OGC Simple Features `Polygon`): an exterior ring, optionally with
/// interior rings ("holes") cut out of it.
///
/// Rings are borrowed slices of points, not owned — see [`crate::line_string::LineString`]
/// for why. Each ring is assumed closed (first point equals last) per the OGC convention;
/// callers building rings by hand must repeat the first point at the end.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Polygon<'a> {
    /// The outer boundary ring.
    pub exterior: &'a [Point],
    /// Interior boundary rings (holes), if any.
    pub interiors: &'a [&'a [Point]],
}

impl<'a> Polygon<'a> {
    /// Constructs a polygon from an exterior ring and zero or more interior rings.
    #[must_use]
    pub const fn new(exterior: &'a [Point], interiors: &'a [&'a [Point]]) -> Self {
        Self { exterior, interiors }
    }

    /// Constructs a polygon with no holes.
    #[must_use]
    pub const fn from_exterior(exterior: &'a [Point]) -> Self {
        Self { exterior, interiors: &[] }
    }

    /// The bounding box enclosing this polygon's exterior ring, or `None` if it is empty.
    #[must_use]
    pub fn bbox(&self) -> Option<Rect> {
        Rect::from_points(self.exterior)
    }

    /// Returns `true` if `point` lies inside this polygon: within the exterior ring and
    /// outside every interior ring (hole).
    #[must_use]
    pub fn contains_point(&self, point: Point) -> bool {
        if !predicates::point_in_ring(point, self.exterior) {
            return false;
        }
        !self.interiors.iter().any(|hole| predicates::point_in_ring(point, hole))
    }

    /// The point on this polygon's boundary (exterior or any interior ring) nearest to
    /// `point` — e.g. the closest point to escape through if `point` is inside the
    /// polygon and it represents a zone to avoid.
    ///
    /// Returns `None` only if the polygon has no rings with at least 2 points.
    #[must_use]
    pub fn nearest_boundary_point(&self, point: Point) -> Option<Point> {
        let mut nearest: Option<(Point, f64)> = None;

        for ring in core::iter::once(self.exterior).chain(self.interiors.iter().copied()) {
            if ring.len() < 2 {
                continue;
            }
            for i in 0..ring.len() {
                let (a, b) = predicates::ring_edges(ring, i);
                let candidate = distance::closest_point_on_segment(point, a, b);
                let d = distance::point_to_point(point, candidate);
                let is_better = match nearest {
                    None => true,
                    Some((_, best_d)) => d < best_d,
                };
                if is_better {
                    nearest = Some((candidate, d));
                }
            }
        }

        nearest.map(|(p, _)| p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_inside_square_is_contained() {
        let square = [
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 10.0),
            Point::new(0.0, 10.0),
            Point::new(0.0, 0.0),
        ];
        let polygon = Polygon::from_exterior(&square);
        assert!(polygon.contains_point(Point::new(5.0, 5.0)));
        assert!(!polygon.contains_point(Point::new(15.0, 5.0)));
    }

    #[test]
    fn point_inside_hole_is_not_contained() {
        let exterior = [
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 10.0),
            Point::new(0.0, 10.0),
            Point::new(0.0, 0.0),
        ];
        let hole = [
            Point::new(4.0, 4.0),
            Point::new(6.0, 4.0),
            Point::new(6.0, 6.0),
            Point::new(4.0, 6.0),
            Point::new(4.0, 4.0),
        ];
        let interiors: [&[Point]; 1] = [&hole];
        let polygon = Polygon::new(&exterior, &interiors);

        assert!(polygon.contains_point(Point::new(1.0, 1.0)));
        assert!(!polygon.contains_point(Point::new(5.0, 5.0)));
    }

    #[test]
    fn nearest_boundary_point_of_interior_point() {
        let square = [
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 10.0),
            Point::new(0.0, 10.0),
            Point::new(0.0, 0.0),
        ];
        let polygon = Polygon::from_exterior(&square);

        // (2, 5) is 2 units from the left edge (x=0) and 8 from the right (x=10) —
        // nearest boundary point should be (0, 5).
        let nearest = polygon.nearest_boundary_point(Point::new(2.0, 5.0)).unwrap();
        assert_eq!(nearest, Point::new(0.0, 5.0));
    }
}
