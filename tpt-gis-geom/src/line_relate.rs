//! Line-line `crosses`: the OGC relation for two line strings whose interiors meet in
//! isolated points only.
//!
//! The boundary of an open line string is its two end points, so an intersection point counts
//! only when it is interior to both lines. A positive-length shared stretch makes the
//! interiors' intersection one-dimensional, which is not `crosses`.
//!
//! Lines are treated as open: their end points are their boundary, even when a line closes on
//! itself.

use crate::float_ext::Fabs as _;
use crate::line_string::LineString;
use crate::point::Point;
use crate::predicates::tolerance_for;

/// How two segments meet.
enum Contact {
    /// No common point.
    None,
    /// A common point.
    Point(Point),
    /// A common stretch of positive length.
    Overlap,
}

fn sub(a: Point, b: Point) -> (f64, f64) {
    (a.x - b.x, a.y - b.y)
}

fn cross(u: (f64, f64), v: (f64, f64)) -> f64 {
    u.0 * v.1 - u.1 * v.0
}

/// How segment `a1→a2` meets segment `b1→b2`.
fn segment_contact(a1: Point, a2: Point, b1: Point, b2: Point) -> Contact {
    let tol = tolerance_for(&[a1, a2, b1, b2]);
    let r = sub(a2, a1);
    let s = sub(b2, b1);
    let len_r = libm::sqrt(r.0 * r.0 + r.1 * r.1);
    let len_s = libm::sqrt(s.0 * s.0 + s.1 * s.1);
    if len_r <= tol || len_s <= tol {
        return Contact::None;
    }
    let denom = cross(r, s);
    let w = sub(b1, a1);
    if denom.fabs() > tol * len_r * len_s {
        // Not parallel: the lines meet at one point; keep it if it lies on both segments.
        let t = cross(w, s) / denom;
        let u = cross(w, r) / denom;
        let slack = tol / len_r.min(len_s);
        if t >= -slack && t <= 1.0 + slack && u >= -slack && u <= 1.0 + slack {
            return Contact::Point(Point::new(a1.x + t * r.0, a1.y + t * r.1));
        }
        return Contact::None;
    }
    // Parallel. Disjoint unless the lines are the same line.
    if cross(w, r).fabs() > tol * len_r {
        return Contact::None;
    }
    // Collinear: measure b's end points along a, as fractions of a's length.
    let project = |p: Point| {
        let d = sub(p, a1);
        (d.0 * r.0 + d.1 * r.1) / (len_r * len_r)
    };
    let (t1, t2) = (project(b1), project(b2));
    let lo = t1.min(t2).max(0.0);
    let hi = t1.max(t2).min(1.0);
    let overlap = (hi - lo) * len_r;
    if overlap > tol {
        Contact::Overlap
    } else if hi >= lo - tol / len_r {
        // They touch at one end point.
        let t = lo.clamp(0.0, 1.0);
        Contact::Point(Point::new(a1.x + t * r.0, a1.y + t * r.1))
    } else {
        Contact::None
    }
}

/// Whether `p` is one of the two end points of a line string.
fn is_end_point(p: Point, line: &[Point]) -> bool {
    let tol = tolerance_for(&[p]);
    let near = |q: Point| (q.x - p.x).fabs() <= tol && (q.y - p.y).fabs() <= tol;
    match (line.first(), line.last()) {
        (Some(&first), Some(&last)) => near(first) || near(last),
        _ => false,
    }
}

/// Returns `true` if two line strings cross in the OGC sense: their interiors meet, in points
/// only, and neither lies along the other.
///
/// Requires at least two points in each line. Line strings that do not meet, touch only at
/// end points, or share a stretch of positive length do not cross.
#[must_use]
pub fn linestrings_cross(a: &LineString, b: &LineString) -> bool {
    if a.points.len() < 2 || b.points.len() < 2 {
        return false;
    }
    let mut interiors_meet = false;
    for sa in a.points.windows(2) {
        for sb in b.points.windows(2) {
            match segment_contact(sa[0], sa[1], sb[0], sb[1]) {
                Contact::None => {}
                // A shared stretch is one-dimensional in the interiors, so not `crosses`.
                Contact::Overlap => return false,
                Contact::Point(p) => {
                    if !is_end_point(p, a.points) && !is_end_point(p, b.points) {
                        interiors_meet = true;
                    }
                }
            }
        }
    }
    interiors_meet
}
