//! Topological predicates: `intersects`, `contains`, and the point-in-polygon test
//! they build on.
//!
//! Floating-point comparisons here use [`EPSILON`] tolerance rather than exact
//! equality, so nearly-collinear points and near-touching segments are classified
//! consistently rather than flipping on rounding noise.

use crate::distance;
use crate::float_ext::Fabs as _;
use crate::line_string::LineString;
use crate::point::Point;
use crate::polygon::Polygon;

/// Tolerance used when classifying near-zero cross products (collinearity) and
/// near-boundary conditions in these predicates. It is a floor: near-boundary tests use
/// `tolerance_for`, which grows with the coordinate magnitude.
pub const EPSILON: f64 = 1e-9;

/// Relative part of the nearness tolerance: about 45 units in the last place of the
/// coordinate magnitude (f64 spacing is about 2.2e-16 relative).
const RELATIVE_TOLERANCE: f64 = 1e-14;

/// The nearness tolerance for a set of points: [`EPSILON`], or [`RELATIVE_TOLERANCE`] times
/// the largest coordinate magnitude if that is larger. Below about 1e5 the absolute floor
/// applies, so ordinary coordinates behave as before. Beyond that, rounding noise grows
/// with magnitude and the absolute floor would fall below it.
fn tolerance_for(points: &[Point]) -> f64 {
    let magnitude = points.iter().fold(0.0_f64, |m, p| m.max(p.x.fabs()).max(p.y.fabs()));
    EPSILON.max(RELATIVE_TOLERANCE * magnitude)
}

/// Returns `true` if `point` lies inside `ring`, using the even-odd (ray casting) rule.
///
/// `ring` is treated as a closed polygon boundary (it does not need to literally repeat
/// its first point as its last — the edge from the last point back to the first is
/// always included).
///
/// The boundary is not classified separately. A point exactly on an edge follows the
/// half-open PNPOLY rule, so the result depends on which side of the ring the edge is on:
/// a point on the left or bottom edge of a square is inside, and one on its right or top
/// edge is outside. Use [`Polygon::contains_point`](crate::Polygon::contains_point) or the
/// OGC predicates when boundary contact should count as covered.
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
    // Tolerance is relative to the size of the two edge vectors, so that the test
    // behaves the same at micrometre scale and at projected-metre scale. An absolute
    // threshold would call every tiny triangle collinear.
    let scale =
        ((q.x - p.x).fabs() + (q.y - p.y).fabs()) * ((r.x - q.x).fabs() + (r.y - q.y).fabs());
    if val.fabs() <= EPSILON * scale {
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
    let tol = tolerance_for(&[p, q, r]);
    q.x <= p.x.max(r.x) + tol
        && q.x >= p.x.min(r.x) - tol
        && q.y <= p.y.max(r.y) + tol
        && q.y >= p.y.min(r.y) - tol
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

/// Returns `true` if a line and a polygon share at least one point: a vertex of the line lies
/// in the polygon (interior or boundary), or a segment of the line meets the boundary.
#[must_use]
pub fn line_intersects_polygon(line: &LineString, poly: &Polygon) -> bool {
    if line.points.iter().any(|&p| covers_point(poly, p)) {
        return true;
    }
    line.points.windows(2).any(|seg| {
        rings(poly).any(|ring| {
            (0..ring.len()).any(|i| {
                let (c, d) = ring_edges(ring, i);
                segments_intersect(seg[0], seg[1], c, d)
            })
        })
    })
}

/// Returns `true` if the two polygons share any point: a boundary edge of one
/// touches or crosses a boundary edge of the other (exterior or hole rings), or one
/// polygon has an exterior vertex inside the other's region.
///
/// Full DE-9IM interior/interior classification is still future work; this covers
/// boundary contact and nesting, and treats holes correctly via
/// [`Polygon::contains_point`].
#[must_use]
pub fn polygons_intersect(a: &Polygon, b: &Polygon) -> bool {
    match (a.bbox(), b.bbox()) {
        (Some(box_a), Some(box_b)) if !box_a.intersects(&box_b) => return false,
        _ => {}
    }

    // Boundary contact: every ring (exterior *and* holes) of each polygon is tested.
    for ring_a in rings(a) {
        for ring_b in rings(b) {
            if ring_segments_intersect(ring_a, ring_b) {
                return true;
            }
        }
    }

    // No boundary contact: either disjoint or nested. A nested polygon's exterior
    // vertices lie strictly inside the other polygon's region.
    if a.exterior.iter().any(|&p| b.contains_point(p)) {
        return true;
    }
    b.exterior.iter().any(|&p| a.contains_point(p))
}

/// Returns `true` if `outer` fully contains `inner` (OGC `contains`): `inner`'s
/// region is a subset of `outer`'s region.
///
/// Boundary contact is allowed. The check is:
/// 1. every exterior vertex of `inner` lies in or on `outer`;
/// 2. no edge of `inner` leaves `outer`: each edge is split at every point where it
///    meets an `outer` ring (including `outer` vertices lying on the edge), and each
///    sub-segment's midpoint must lie in or on `outer`, so holes in `outer` are
///    respected;
/// 3. no hole of `outer` pokes into `inner`'s region.
///
/// Rings are assumed simple and closed. Points exactly on the boundary are treated
/// as covered (OGC-style). If an edge is cut more than `MAX_CUTS` times the
/// function conservatively returns `false`.
#[must_use]
pub fn polygon_contains_polygon(outer: &Polygon, inner: &Polygon) -> bool {
    if !inner.exterior.iter().all(|&p| covers_point(outer, p)) {
        return false;
    }

    for i in 0..inner.exterior.len() {
        let (p, q) = ring_edges(inner.exterior, i);
        let mut cuts = Cuts::new();
        cuts.push(0.0);
        cuts.push(1.0);
        for ring in rings(outer) {
            for j in 0..ring.len() {
                let (a, b) = ring_edges(ring, j);
                if let Some(t) = proper_crossing_param(p, q, a, b) {
                    if !cuts.push(t) {
                        return false;
                    }
                }
                if let Some(t) = vertex_param_on_segment(ring[j], p, q) {
                    if !cuts.push(t) {
                        return false;
                    }
                }
            }
        }
        let sorted = cuts.sorted();
        for pair in sorted.windows(2) {
            if pair[1] <= pair[0] {
                continue;
            }
            let mid_t = 0.5 * (pair[0] + pair[1]);
            let mid = Point::new(p.x + mid_t * (q.x - p.x), p.y + mid_t * (q.y - p.y));
            if !covers_point(outer, mid) {
                return false;
            }
        }
    }

    // A hole of `outer` with a vertex strictly inside `inner` means part of `inner`
    // lies in a region `outer` does not cover.
    !outer.interiors.iter().any(|hole| hole.iter().any(|&v| strictly_inside(inner, v)))
}

/// Iterates over a polygon's exterior ring followed by its hole rings.
pub(crate) fn rings<'a>(poly: &Polygon<'a>) -> impl Iterator<Item = &'a [Point]> + Clone + 'a {
    core::iter::once(poly.exterior).chain(poly.interiors.iter().copied())
}

/// `true` if `p` is in `poly`'s closed region: inside, or on any of its rings.
pub(crate) fn covers_point(poly: &Polygon, p: Point) -> bool {
    poly.contains_point(p) || on_polygon_boundary(poly, p)
}

/// `true` if `p` is on the boundary of `poly` (within [`tolerance_for`] of the polygon's
/// coordinates).
pub(crate) fn on_polygon_boundary(poly: &Polygon, p: Point) -> bool {
    rings(poly).any(|ring| {
        (0..ring.len()).any(|i| {
            let (a, b) = ring_edges(ring, i);
            distance::point_to_segment(p, a, b) <= tolerance_for(&[p, a, b])
        })
    })
}

/// `true` if `p` is inside `poly`'s region and not on its boundary.
pub(crate) fn strictly_inside(poly: &Polygon, p: Point) -> bool {
    poly.contains_point(p) && !on_polygon_boundary(poly, p)
}

/// The parameter `t` in `(0, 1)` at which segment `p→q` properly crosses segment
/// `a→b` (interior to both), or `None` if they do not cross there. Endpoint touches
/// are left to the vertex test.
fn proper_crossing_param(p: Point, q: Point, a: Point, b: Point) -> Option<f64> {
    let r = (q.x - p.x, q.y - p.y);
    let s = (b.x - a.x, b.y - a.y);
    let denom = r.0 * s.1 - r.1 * s.0;
    if denom.fabs() < f64::EPSILON {
        return None;
    }
    let ap = (a.x - p.x, a.y - p.y);
    let t = (ap.0 * s.1 - ap.1 * s.0) / denom;
    let u = (ap.0 * r.1 - ap.1 * r.0) / denom;
    (t > 0.0 && t < 1.0 && u > 0.0 && u < 1.0).then_some(t)
}

/// If `v` lies on segment `p→q` strictly between its endpoints (within [`EPSILON`]),
/// the parameter `t` in `(0, 1)` of that point.
fn vertex_param_on_segment(v: Point, p: Point, q: Point) -> Option<f64> {
    let d = (q.x - p.x, q.y - p.y);
    let len_sq = d.0 * d.0 + d.1 * d.1;
    if len_sq == 0.0 {
        return None;
    }
    let t = ((v.x - p.x) * d.0 + (v.y - p.y) * d.1) / len_sq;
    if t <= 0.0 || t >= 1.0 {
        return None;
    }
    (distance::point_to_segment(v, p, q) <= EPSILON).then_some(t)
}

/// Maximum number of split points tracked along one edge. Exceeding it makes
/// [`polygon_contains_polygon`] answer `false` rather than allocate.
const MAX_CUTS: usize = 128;

/// A fixed-capacity, allocation-free buffer of split parameters along one edge, so
/// the predicates remain usable in `no_std`/`alloc`-free builds.
struct Cuts {
    values: [f64; MAX_CUTS],
    len: usize,
}

impl Cuts {
    fn new() -> Self {
        Self { values: [0.0; MAX_CUTS], len: 0 }
    }

    /// Adds a parameter; returns `false` if the buffer is full.
    fn push(&mut self, t: f64) -> bool {
        if self.len == MAX_CUTS {
            return false;
        }
        self.values[self.len] = t;
        self.len += 1;
        true
    }

    /// Sorts the stored parameters in place (insertion sort; the buffer is small).
    fn sorted(&mut self) -> &[f64] {
        let slice = &mut self.values[..self.len];
        for i in 1..slice.len() {
            let mut j = i;
            while j > 0 && slice[j - 1] > slice[j] {
                slice.swap(j - 1, j);
                j -= 1;
            }
        }
        &self.values[..self.len]
    }
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

    /// Segments at micrometre scale must still be classified correctly; an absolute
    /// orientation tolerance made every such triangle look collinear.
    #[test]
    fn crossing_segments_are_detected_at_tiny_scale() {
        let s = 1e-7;
        assert!(segments_intersect(
            Point::new(0.0, 0.0),
            Point::new(4.0 * s, 4.0 * s),
            Point::new(0.0, 4.0 * s),
            Point::new(4.0 * s, 0.0),
        ));
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

    /// OGC `contains` permits the inner boundary to touch the outer boundary.
    #[test]
    fn contains_allows_shared_boundary() {
        let outer = square(0.0, 10.0);
        let inner = square(0.0, 5.0);
        let poly_outer = Polygon::from_exterior(&outer);
        let poly_inner = Polygon::from_exterior(&inner);
        assert!(polygon_contains_polygon(&poly_outer, &poly_inner));
    }

    /// A polygon sitting in the hole of `outer` is not contained by it.
    #[test]
    fn contains_rejects_polygon_inside_hole() {
        let outer = square(0.0, 10.0);
        let hole = square(2.0, 8.0);
        let holes: [&[Point]; 1] = [&hole];
        let poly_outer = Polygon::new(&outer, &holes);
        let inner = square(4.0, 6.0);
        let poly_inner = Polygon::from_exterior(&inner);
        assert!(!polygon_contains_polygon(&poly_outer, &poly_inner));
        assert!(!polygons_intersect(&poly_outer, &poly_inner));
    }

    /// An inner polygon that swallows an outer hole does not fit inside `outer`:
    /// the hole's region is not part of `outer`.
    #[test]
    fn contains_rejects_polygon_that_covers_a_hole() {
        let outer = square(0.0, 10.0);
        let hole = square(4.0, 6.0);
        let holes: [&[Point]; 1] = [&hole];
        let poly_outer = Polygon::new(&outer, &holes);
        let inner = square(3.0, 7.0);
        let poly_inner = Polygon::from_exterior(&inner);
        assert!(!polygon_contains_polygon(&poly_outer, &poly_inner));
    }

    /// Containment must hold even when the inner ring's edges cross the outer
    /// boundary exactly at an outer vertex and continue outward.
    #[test]
    fn contains_rejects_edge_leaving_through_outer_vertex() {
        let outer = [
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(5.0, 5.0),
            Point::new(0.0, 10.0),
            Point::new(0.0, 0.0),
        ];
        // Vertex (5,5) of `outer` is a notch; this triangle's edge passes through it
        // and exits to (12, 12), well outside.
        let inner = [
            Point::new(1.0, 1.0),
            Point::new(12.0, 12.0),
            Point::new(1.0, 2.0),
            Point::new(1.0, 1.0),
        ];
        let poly_outer = Polygon::from_exterior(&outer);
        let poly_inner = Polygon::from_exterior(&inner);
        assert!(!polygon_contains_polygon(&poly_outer, &poly_inner));
    }
}

#[cfg(test)]
mod tolerance_tests {
    use super::*;

    #[test]
    fn point_in_ring_boundary_is_half_open() {
        let square = [
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 10.0),
            Point::new(0.0, 10.0),
        ];
        // Left and bottom edges count as inside; right and top edges as outside.
        assert!(point_in_ring(Point::new(0.0, 5.0), &square));
        assert!(point_in_ring(Point::new(5.0, 0.0), &square));
        assert!(!point_in_ring(Point::new(10.0, 5.0), &square));
        assert!(!point_in_ring(Point::new(5.0, 10.0), &square));
    }

    /// Two collinear segments that meet within one unit of rounding at 1e9 scale. An
    /// absolute tolerance of 1e-9 is below that spacing and would call them disjoint.
    #[test]
    fn segments_touch_within_rounding_at_large_magnitude() {
        let big = 1.0e9_f64;
        let ulp = f64::from_bits(big.to_bits() + 1) - big; // spacing of doubles at 1e9
        let a1 = Point::new(0.0, 0.0);
        let a2 = Point::new(big, big);
        let b1 = Point::new(big + ulp * 8.0, big);
        let b2 = Point::new(2.0 * big, 2.0 * big);
        assert!(segments_intersect(a1, a2, b1, b2));
        // Far apart at the same magnitude: still disjoint.
        let c1 = Point::new(big + 1.0, big);
        let c2 = Point::new(2.0 * big, 2.0 * big);
        assert!(!segments_intersect(a1, a2, c1, c2));
    }
}
