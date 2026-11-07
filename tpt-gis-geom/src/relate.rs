//! Polygon-polygon `touches` and `overlaps` relations, built on the boolean operations in
//! [`crate::clip`]. Requires the `alloc` feature.
//!
//! These are the common cases of the OGC relations, not the full DE-9IM matrix:
//!
//! - [`polygons_touch`]: the polygons meet (share a point) but their interiors do not
//!   overlap, meaning their intersection has no area.
//! - [`polygons_overlap`]: the intersection has area, and neither polygon contains the
//!   other.
//!
//! Area comparisons use a relative tolerance of `1e-9` of the combined area.

use crate::clip;
use crate::float_ext::Fabs as _;
use crate::line_relate::{is_end_point, segment_contact, sub, Contact};
use crate::line_string::LineString;
use crate::point::Point;
use crate::polygon::Polygon;
use crate::predicates::{
    covers_point, polygon_contains_polygon, polygons_intersect, ring_edges, rings,
    segments_intersect, strictly_inside, tolerance_for,
};
use alloc::vec::Vec;

/// Relative tolerance for "no area" when comparing against the combined area.
const AREA_TOLERANCE: f64 = 1e-9;

fn intersection_area(a: &Polygon, b: &Polygon) -> f64 {
    clip::intersection(a, b).iter().map(clip::OwnedPolygon::area).sum()
}

/// Returns `true` if the two polygons share at least one point but their interiors do
/// not overlap.
#[must_use]
pub fn polygons_touch(a: &Polygon, b: &Polygon) -> bool {
    if !polygons_intersect(a, b) {
        return false;
    }
    let scale = a.area().fabs() + b.area().fabs();
    intersection_area(a, b) <= AREA_TOLERANCE * scale
}

/// Returns `true` if the interiors of the two polygons overlap and neither contains the
/// other.
#[must_use]
pub fn polygons_overlap(a: &Polygon, b: &Polygon) -> bool {
    if polygon_contains_polygon(a, b) || polygon_contains_polygon(b, a) {
        return false;
    }
    let scale = a.area().fabs() + b.area().fabs();
    intersection_area(a, b) > AREA_TOLERANCE * scale
}

/// The parameter `t` in `[0, 1]` along `a→b` where it meets `c→d`. For a collinear overlap,
/// the parameters of the overlap's endpoints that lie on `a→b` are returned as well.
fn crossing_params(a: Point, b: Point, c: Point, d: Point, out: &mut Vec<f64>) {
    if !segments_intersect(a, b, c, d) {
        return;
    }
    let r = (b.x - a.x, b.y - a.y);
    let s = (d.x - c.x, d.y - c.y);
    let denom = r.0 * s.1 - r.1 * s.0;
    if denom == 0.0 {
        // Parallel and touching: keep the endpoints of `c→d` that lie within `a→b`.
        let len_sq = r.0 * r.0 + r.1 * r.1;
        if len_sq == 0.0 {
            return;
        }
        for p in [c, d] {
            let t = ((p.x - a.x) * r.0 + (p.y - a.y) * r.1) / len_sq;
            if (0.0..=1.0).contains(&t) {
                out.push(t);
            }
        }
        return;
    }
    let ca = (c.x - a.x, c.y - a.y);
    let t = (ca.0 * s.1 - ca.1 * s.0) / denom;
    if (0.0..=1.0).contains(&t) {
        out.push(t);
    }
}

/// Classifies the parts of a line against a polygon: `(some part strictly inside, some part
/// strictly outside)`. The line is split where it meets the boundary, and each piece is
/// classified by its midpoint. Parts on the boundary are neither.
fn classify_line_parts(line: &LineString, poly: &Polygon) -> (bool, bool) {
    let mut cuts: Vec<f64> = Vec::new();
    let (mut inside, mut outside) = (false, false);
    for seg in line.points.windows(2) {
        let (a, b) = (seg[0], seg[1]);
        cuts.clear();
        cuts.push(0.0);
        cuts.push(1.0);
        for ring in rings(poly) {
            for i in 0..ring.len() {
                let (c, d) = ring_edges(ring, i);
                crossing_params(a, b, c, d, &mut cuts);
            }
        }
        cuts.sort_by(f64::total_cmp);
        for pair in cuts.windows(2) {
            if pair[1] - pair[0] <= 1e-12 {
                continue;
            }
            let t = 0.5 * (pair[0] + pair[1]);
            let mid = Point::new(a.x + t * (b.x - a.x), a.y + t * (b.y - a.y));
            if strictly_inside(poly, mid) {
                inside = true;
            } else if !covers_point(poly, mid) {
                outside = true;
            }
            if inside && outside {
                return (true, true);
            }
        }
    }
    (inside, outside)
}

/// Returns `true` if a line crosses a polygon in the OGC sense: some part of the line lies in
/// the polygon's interior and some part lies outside the polygon, so the two interiors meet
/// and neither contains the other. Parts on the boundary do not count either way.
///
/// The line is split where it meets the boundary, and each piece is classified by its
/// midpoint. Requires the `alloc` feature.
#[must_use]
pub fn line_crosses_polygon(line: &LineString, poly: &Polygon) -> bool {
    let (inside, outside) = classify_line_parts(line, poly);
    inside && outside
}

/// Returns `true` if a line lies within a polygon in the OGC sense: every part is in the
/// polygon's closure, and some part is in its interior. A line along the boundary alone is
/// not within. Requires the `alloc` feature.
#[must_use]
pub fn line_within_polygon(line: &LineString, poly: &Polygon) -> bool {
    let (inside, outside) = classify_line_parts(line, poly);
    inside && !outside
}

/// Returns `true` if every point of `a` lies on `b` and some point of `a` lies in the interior
/// of `b`: the OGC `within` relation for two line strings.
///
/// Each segment of `a` is split where it meets `b`, and each piece's midpoint is tested against
/// `b`. A piece on `b` that is not at `b`'s end points gives interior contact.
#[must_use]
pub fn linestring_within_linestring(a: &LineString, b: &LineString) -> bool {
    if a.points.len() < 2 || b.points.len() < 2 {
        return false;
    }
    let mut interiors_meet = false;
    let mut cuts: Vec<f64> = Vec::new();
    for sa in a.points.windows(2) {
        let (p, q) = (sa[0], sa[1]);
        let r = sub(q, p);
        let len_sq = r.0 * r.0 + r.1 * r.1;
        cuts.clear();
        cuts.push(0.0);
        cuts.push(1.0);
        if len_sq == 0.0 {
            continue;
        }
        let param = |x: Point| {
            let d = sub(x, p);
            ((d.0 * r.0 + d.1 * r.1) / len_sq).clamp(0.0, 1.0)
        };
        for sb in b.points.windows(2) {
            match segment_contact(p, q, sb[0], sb[1]) {
                Contact::None => {}
                Contact::Point(x) => cuts.push(param(x)),
                Contact::Overlap(x, y) => {
                    cuts.push(param(x));
                    cuts.push(param(y));
                }
            }
        }
        cuts.sort_by(f64::total_cmp);
        for pair in cuts.windows(2) {
            if pair[1] - pair[0] <= 1e-12 {
                continue;
            }
            let t = 0.5 * (pair[0] + pair[1]);
            let mid = Point::new(p.x + t * r.0, p.y + t * r.1);
            if !on_line(mid, b.points) {
                return false;
            }
            if !is_end_point(mid, b.points) {
                interiors_meet = true;
            }
        }
    }
    interiors_meet
}

/// Whether `p` lies on some segment of the polyline `line`, within tolerance.
fn on_line(p: Point, line: &[Point]) -> bool {
    let tol = tolerance_for(&[p]);
    line.windows(2).any(|w| crate::distance::point_to_segment(p, w[0], w[1]) <= tol)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::point::Point;

    fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> [Point; 5] {
        [
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
            Point::new(x0, y0),
        ]
    }

    #[test]
    fn overlapping_squares_overlap_but_do_not_touch() {
        let (a, b) = (square(0.0, 0.0, 10.0, 10.0), square(5.0, 5.0, 15.0, 15.0));
        let (pa, pb) = (Polygon::from_exterior(&a), Polygon::from_exterior(&b));
        assert!(polygons_overlap(&pa, &pb));
        assert!(!polygons_touch(&pa, &pb));
    }

    #[test]
    fn shared_edge_touches_but_does_not_overlap() {
        let (a, b) = (square(0.0, 0.0, 10.0, 10.0), square(10.0, 0.0, 20.0, 10.0));
        let (pa, pb) = (Polygon::from_exterior(&a), Polygon::from_exterior(&b));
        assert!(polygons_touch(&pa, &pb));
        assert!(!polygons_overlap(&pa, &pb));
    }

    #[test]
    fn corner_contact_touches() {
        let (a, b) = (square(0.0, 0.0, 10.0, 10.0), square(10.0, 10.0, 20.0, 20.0));
        let (pa, pb) = (Polygon::from_exterior(&a), Polygon::from_exterior(&b));
        assert!(polygons_touch(&pa, &pb));
        assert!(!polygons_overlap(&pa, &pb));
    }

    #[test]
    fn nested_polygons_neither_touch_nor_overlap() {
        let (a, b) = (square(0.0, 0.0, 10.0, 10.0), square(2.0, 2.0, 4.0, 4.0));
        let (pa, pb) = (Polygon::from_exterior(&a), Polygon::from_exterior(&b));
        assert!(!polygons_overlap(&pa, &pb));
        assert!(!polygons_touch(&pa, &pb));
    }

    #[test]
    fn disjoint_polygons_neither_touch_nor_overlap() {
        let (a, b) = (square(0.0, 0.0, 1.0, 1.0), square(5.0, 5.0, 6.0, 6.0));
        let (pa, pb) = (Polygon::from_exterior(&a), Polygon::from_exterior(&b));
        assert!(!polygons_overlap(&pa, &pb));
        assert!(!polygons_touch(&pa, &pb));
    }
}
