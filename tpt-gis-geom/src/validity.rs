//! Basic OGC-style validity checks for polygons.
//!
//! [`Polygon::is_valid`] covers the checks that matter for the rest of this crate:
//! rings are closed and have at least 3 distinct edges, no ring has zero area, no ring
//! crosses or touches itself except at shared endpoints of adjacent edges, and every
//! hole lies within the exterior. Ring-to-ring touching (e.g. a hole sharing an edge
//! with the exterior) is reported as invalid only when it is a crossing; this is
//! documented on the method.

use crate::measure::ring_signed_area;
use crate::point::Point;
use crate::polygon::Polygon;
use crate::predicates::{polygon_contains_polygon, segments_intersect, EPSILON};

/// Checks one closed ring: at least four points (three distinct corners plus the
/// repeated first point), first equal to last, non-zero area, and no self-intersection
/// between non-adjacent edges.
fn ring_is_simple(ring: &[Point]) -> bool {
    if ring.len() < 4 || ring.first() != ring.last() {
        return false;
    }
    if ring_signed_area(ring).abs() <= EPSILON {
        return false;
    }
    // Edges e_i = (ring[i], ring[i+1]) for i in 0..m, where m = len - 1 (the last point
    // repeats the first, so there are m real edges).
    let m = ring.len() - 1;
    for i in 0..m {
        for j in (i + 2)..m {
            // The first and last edges share the closing vertex: they are adjacent.
            if i == 0 && j == m - 1 {
                continue;
            }
            if segments_intersect(ring[i], ring[i + 1], ring[j], ring[j + 1]) {
                return false;
            }
        }
    }
    true
}

impl Polygon<'_> {
    /// Returns `true` if the polygon passes the basic validity checks described in the
    /// [module documentation](self).
    ///
    /// This is not a full OGC `IsValid` implementation: it does not detect a hole that
    /// touches the exterior at a single point, or a hole that touches another hole.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        if !ring_is_simple(self.exterior) {
            return false;
        }
        for hole in self.interiors.iter() {
            if !ring_is_simple(hole) {
                return false;
            }
            // A hole must sit inside the exterior ring.
            let exterior_only = Polygon::from_exterior(self.exterior);
            let hole_poly = Polygon::from_exterior(hole);
            if !polygon_contains_polygon(&exterior_only, &hole_poly) {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_square_is_valid() {
        let sq = [
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(4.0, 4.0),
            Point::new(0.0, 4.0),
            Point::new(0.0, 0.0),
        ];
        assert!(Polygon::from_exterior(&sq).is_valid());
    }

    #[test]
    fn bowtie_is_invalid() {
        // Edges (0,0)-(4,4) and (4,0)-(0,4) cross at (2,2).
        let bowtie = [
            Point::new(0.0, 0.0),
            Point::new(4.0, 4.0),
            Point::new(4.0, 0.0),
            Point::new(0.0, 4.0),
            Point::new(0.0, 0.0),
        ];
        assert!(!Polygon::from_exterior(&bowtie).is_valid());
    }

    #[test]
    fn unclosed_ring_is_invalid() {
        let open = [
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(4.0, 4.0),
            Point::new(0.0, 4.0),
        ];
        assert!(!Polygon::from_exterior(&open).is_valid());
    }

    #[test]
    fn hole_outside_exterior_is_invalid() {
        let sq = [
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(4.0, 4.0),
            Point::new(0.0, 4.0),
            Point::new(0.0, 0.0),
        ];
        let hole = [
            Point::new(10.0, 10.0),
            Point::new(11.0, 10.0),
            Point::new(11.0, 11.0),
            Point::new(10.0, 11.0),
            Point::new(10.0, 10.0),
        ];
        let holes: [&[Point]; 1] = [&hole];
        assert!(!Polygon::new(&sq, &holes).is_valid());
    }

    #[test]
    fn hole_inside_exterior_is_valid() {
        let sq = [
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
        let holes: [&[Point]; 1] = [&hole];
        assert!(Polygon::new(&sq, &holes).is_valid());
    }
}
