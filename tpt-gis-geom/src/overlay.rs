//! Segment intersection points and the `within` relation for polygons.
//!
//! These build on the boolean predicates in [`crate::predicates`]. They do not compute
//! general polygon overlay (union, intersection, difference); see the crate docs.

use crate::distance::point_to_segment;
use crate::point::Point;
use crate::polygon::Polygon;
use crate::predicates::{polygon_contains_polygon, segments_intersect, EPSILON};

fn cross(a: Point, b: Point) -> f64 {
    a.x * b.y - a.y * b.x
}

/// The point where segments `p1`-`q1` and `p2`-`q2` meet, if they do.
///
/// For a proper crossing this is the unique intersection point. When the segments are
/// collinear and overlap, one of the shared endpoints is returned. Segments that only
/// touch at an endpoint return that endpoint.
#[must_use]
pub fn segment_intersection(p1: Point, q1: Point, p2: Point, q2: Point) -> Option<Point> {
    if !segments_intersect(p1, q1, p2, q2) {
        return None;
    }
    let r = Point::new(q1.x - p1.x, q1.y - p1.y);
    let s = Point::new(q2.x - p2.x, q2.y - p2.y);
    let denom = cross(r, s);

    // Parallel when sin²θ = denom² / (|r|²|s|²) is negligible. Squared form avoids
    // `sqrt`/`powi`, which are not available without `std`.
    let r_len_sq = r.x * r.x + r.y * r.y;
    let s_len_sq = s.x * s.x + s.y * s.y;
    if denom * denom <= f64::EPSILON * f64::EPSILON * r_len_sq * s_len_sq {
        // Parallel (collinear, since they intersect): return an endpoint that lies on
        // the other segment.
        for p in [p2, q2] {
            if point_to_segment(p, p1, q1) <= EPSILON {
                return Some(p);
            }
        }
        for p in [p1, q1] {
            if point_to_segment(p, p2, q2) <= EPSILON {
                return Some(p);
            }
        }
        return None;
    }

    let qp = Point::new(p2.x - p1.x, p2.y - p1.y);
    let t = cross(qp, s) / denom;
    Some(Point::new(p1.x + t * r.x, p1.y + t * r.y))
}

/// Returns `true` if polygon `inner` lies within polygon `outer` (the OGC `within`
/// relation, which is the converse of `contains`). Boundary contact is allowed.
#[must_use]
pub fn polygon_within(inner: &Polygon, outer: &Polygon) -> bool {
    polygon_contains_polygon(outer, inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::float_ext::Fabs as _;

    #[test]
    fn crossing_segments_meet_at_centre() {
        let p = segment_intersection(
            Point::new(0.0, 0.0),
            Point::new(4.0, 4.0),
            Point::new(0.0, 4.0),
            Point::new(4.0, 0.0),
        )
        .unwrap();
        assert!((p.x - 2.0).fabs() < 1e-9 && (p.y - 2.0).fabs() < 1e-9);
    }

    #[test]
    fn disjoint_segments_have_no_intersection() {
        assert_eq!(
            segment_intersection(
                Point::new(0.0, 0.0),
                Point::new(1.0, 0.0),
                Point::new(0.0, 1.0),
                Point::new(1.0, 1.0),
            ),
            None
        );
    }

    #[test]
    fn endpoint_touch_returns_that_endpoint() {
        let p = segment_intersection(
            Point::new(0.0, 0.0),
            Point::new(2.0, 0.0),
            Point::new(2.0, 0.0),
            Point::new(2.0, 5.0),
        )
        .unwrap();
        assert_eq!(p, Point::new(2.0, 0.0));
    }

    #[test]
    fn collinear_overlap_returns_a_shared_point() {
        let p = segment_intersection(
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(2.0, 0.0),
            Point::new(6.0, 0.0),
        )
        .unwrap();
        assert!(p.y.fabs() < 1e-9 && p.x >= 2.0 - 1e-9 && p.x <= 4.0 + 1e-9);
    }

    #[test]
    fn within_is_the_converse_of_contains() {
        let outer_ring = [
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 10.0),
            Point::new(0.0, 10.0),
            Point::new(0.0, 0.0),
        ];
        let inner_ring = [
            Point::new(2.0, 2.0),
            Point::new(4.0, 2.0),
            Point::new(4.0, 4.0),
            Point::new(2.0, 4.0),
            Point::new(2.0, 2.0),
        ];
        let outer = Polygon::from_exterior(&outer_ring);
        let inner = Polygon::from_exterior(&inner_ring);
        assert!(polygon_within(&inner, &outer));
        assert!(!polygon_within(&outer, &inner));
    }
}
