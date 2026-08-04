//! Topological predicates: `intersects`, `contains`, and the point-in-polygon test
//! they build on.
//!
//! Floating-point comparisons here use [`EPSILON`] tolerance rather than exact
//! equality, so nearly-collinear points and near-touching segments are classified
//! consistently rather than flipping on rounding noise.

use crate::line_string::LineString;
use crate::point::Point;
use crate::polygon::Polygon;

/// Tolerance used when classifying near-zero cross products (collinearity) and
/// near-boundary conditions in these predicates.
pub const EPSILON: f64 = 1e-9;

/// Returns `true` if `point` lies inside `ring`, using the even-odd (ray casting) rule.
///
/// `ring` is treated as a closed polygon boundary (it does not need to literally repeat
/// its first point as its last — the edge from the last point back to the first is
/// always included). Points exactly on the boundary may be classified as inside or
/// outside depending on which edge they fall on; this matches the standard PNPOLY
/// algorithm's well-known behavior.
#[must_use]
pub fn point_in_ring(point: Point, ring: &[Point]) -> bool {
    let n = ring.len();
    if n < 3 {
        return false;
    }

    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let vi = ring[i];
        let vj = ring[j];
        if ((vi.y > point.y) != (vj.y > point.y))
            && (point.x < (vj.x - vi.x) * (point.y - vi.y) / (vj.y - vi.y) + vi.x)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// The orientation of the ordered triple `(p, q, r)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Orientation {
    Collinear,
    Clockwise,
    CounterClockwise,
}

fn orientation(p: Point, q: Point, r: Point) -> Orientation {
    let val = (q.y - p.y) * (r.x - q.x) - (q.x - p.x) * (r.y - q.y);
    if val.abs() < EPSILON {
        Orientation::Collinear
    } else if val > 0.0 {
        Orientation::Clockwise
    } else {
        Orientation::CounterClockwise
    }
}

/// Returns `true` if `q` lies on the segment `p`-`r`, given that `p`, `q`, `r` are
/// already known to be collinear.
fn on_segment(p: Point, q: Point, r: Point) -> bool {
    q.x <= p.x.max(r.x) + EPSILON
        && q.x >= p.x.min(r.x) - EPSILON
        && q.y <= p.y.max(r.y) + EPSILON
        && q.y >= p.y.min(r.y) - EPSILON
}

/// Returns `true` if segments `p1`-`q1` and `p2`-`q2` intersect or touch.
#[must_use]
pub fn segments_intersect(p1: Point, q1: Point, p2: Point, q2: Point) -> bool {
    let o1 = orientation(p1, q1, p2);
    let o2 = orientation(p1, q1, q2);
    let o3 = orientation(p2, q2, p1);
    let o4 = orientation(p2, q2, q1);

    if o1 != o2 && o3 != o4 {
        return true;
    }

    (o1 == Orientation::Collinear && on_segment(p1, p2, q1))
        || (o2 == Orientation::Collinear && on_segment(p1, q2, q1))
        || (o3 == Orientation::Collinear && on_segment(p2, p1, q2))
        || (o4 == Orientation::Collinear && on_segment(p2, q1, q2))
}

/// Returns ring edge `i` as `(start, end)`, wrapping the last edge back to the first
/// point (rings are implicitly closed, regardless of whether the caller's slice
/// literally repeats its first point at the end).
pub(crate) fn ring_edges(ring: &[Point], i: usize) -> (Point, Point) {
    (ring[i], ring[(i + 1) % ring.len()])
}

fn ring_segments_intersect(a: &[Point], b: &[Point]) -> bool {
    if a.is_empty() || b.is_empty() {
        return false;
    }
    for i in 0..a.len() {
        let (a1, a2) = ring_edges(a, i);
        for j in 0..b.len() {
            let (b1, b2) = ring_edges(b, j);
            if segments_intersect(a1, a2, b1, b2) {
                return true;
            }
        }
    }
    false
}

/// Returns `true` if the two line strings share at least one point.
#[must_use]
pub fn linestrings_intersect(a: &LineString, b: &LineString) -> bool {
    for (a1, a2) in a.segments() {
        for (b1, b2) in b.segments() {
            if segments_intersect(a1, a2, b1, b2) {
                return true;
            }
        }
    }
    false
}

/// Returns `true` if the two polygons' exteriors overlap: their boundaries cross, or
/// one polygon has a vertex inside the other.
///
/// This covers the common OGC "intersects" cases (boundary crossing and full
/// containment) but does not attempt exact interior/interior overlap classification
/// for polygons with no crossing edges and no vertex nested inside the other purely
/// via boundary/hole interactions — full topological overlay (per DE-9IM) is future work.
#[must_use]
pub fn polygons_intersect(a: &Polygon, b: &Polygon) -> bool {
    match (a.bbox(), b.bbox()) {
        (Some(box_a), Some(box_b)) if !box_a.intersects(&box_b) => return false,
        _ => {}
    }

    if ring_segments_intersect(a.exterior, b.exterior) {
        return true;
    }
    if a.exterior.iter().any(|&p| b.contains_point(p)) {
        return true;
    }
    if b.exterior.iter().any(|&p| a.contains_point(p)) {
        return true;
    }
    false
}

/// Returns `true` if `outer` fully contains `inner`: every vertex of `inner`'s exterior
/// lies inside `outer`, and their boundaries do not cross.
#[must_use]
pub fn polygon_contains_polygon(outer: &Polygon, inner: &Polygon) -> bool {
    if ring_segments_intersect(outer.exterior, inner.exterior) {
        return false;
    }
    inner.exterior.iter().all(|&p| outer.contains_point(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(min: f64, max: f64) -> [Point; 5] {
        [
            Point::new(min, min),
            Point::new(max, min),
            Point::new(max, max),
            Point::new(min, max),
            Point::new(min, min),
        ]
    }

    #[test]
    fn point_in_ring_basic_square() {
        let sq = square(0.0, 10.0);
        assert!(point_in_ring(Point::new(5.0, 5.0), &sq));
        assert!(!point_in_ring(Point::new(15.0, 5.0), &sq));
    }

    #[test]
    fn crossing_segments_intersect() {
        let p1 = Point::new(0.0, 0.0);
        let q1 = Point::new(4.0, 4.0);
        let p2 = Point::new(0.0, 4.0);
        let q2 = Point::new(4.0, 0.0);
        assert!(segments_intersect(p1, q1, p2, q2));
    }

    #[test]
    fn parallel_non_touching_segments_do_not_intersect() {
        let p1 = Point::new(0.0, 0.0);
        let q1 = Point::new(4.0, 0.0);
        let p2 = Point::new(0.0, 1.0);
        let q2 = Point::new(4.0, 1.0);
        assert!(!segments_intersect(p1, q1, p2, q2));
    }

    #[test]
    fn overlapping_squares_intersect() {
        let a = square(0.0, 10.0);
        let b = square(5.0, 15.0);
        let poly_a = Polygon::from_exterior(&a);
        let poly_b = Polygon::from_exterior(&b);
        assert!(polygons_intersect(&poly_a, &poly_b));
    }

    #[test]
    fn disjoint_squares_do_not_intersect() {
        let a = square(0.0, 1.0);
        let b = square(10.0, 11.0);
        let poly_a = Polygon::from_exterior(&a);
        let poly_b = Polygon::from_exterior(&b);
        assert!(!polygons_intersect(&poly_a, &poly_b));
    }

    #[test]
    fn nested_square_is_contained() {
        let outer = square(0.0, 10.0);
        let inner = square(2.0, 8.0);
        let poly_outer = Polygon::from_exterior(&outer);
        let poly_inner = Polygon::from_exterior(&inner);
        assert!(polygon_contains_polygon(&poly_outer, &poly_inner));
        assert!(polygons_intersect(&poly_outer, &poly_inner));
    }
}
