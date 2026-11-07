//! Convex hull of a point set (Andrew's monotone chain).
//!
//! Allocation-free, so it works in `no_std` builds: the caller supplies the working
//! buffer and the output buffer.

use crate::point::Point;

/// Cross product of `o→a` and `o→b`: positive when `o, a, b` turn counter-clockwise.
fn cross(o: Point, a: Point, b: Point) -> f64 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

/// Computes the convex hull of `points`, writing its vertices into `out` in
/// counter-clockwise order (with no repeated closing point).
///
/// `points` is used as scratch space and is reordered (sorted by `x`, then `y`).
/// Returns the number of hull vertices written, or `None` if `out` is shorter than
/// `2 * points.len()` (the worst-case buffer size for the monotone chain). Collinear
/// points on the hull boundary are dropped. For fewer than three input points, the
/// points themselves are returned.
///
/// # Examples
///
/// ```
/// use tpt_gis_geom::{hull::convex_hull, Point};
///
/// let mut pts = [
///     Point::new(0.0, 0.0), Point::new(4.0, 0.0), Point::new(4.0, 4.0),
///     Point::new(0.0, 4.0), Point::new(2.0, 2.0), // interior point
/// ];
/// let mut out = [Point::new(0.0, 0.0); 10];
/// let n = convex_hull(&mut pts, &mut out).unwrap();
/// assert_eq!(n, 4);
/// ```
pub fn convex_hull(points: &mut [Point], out: &mut [Point]) -> Option<usize> {
    let n = points.len();
    if n < 3 {
        if out.len() < n {
            return None;
        }
        out[..n].copy_from_slice(points);
        return Some(n);
    }
    if out.len() < 2 * n {
        return None;
    }

    points.sort_unstable_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));

    // Lower hull, then upper hull, each popping points that do not turn left.
    let mut k = 0;
    for &p in points.iter() {
        while k >= 2 && cross(out[k - 2], out[k - 1], p) <= 0.0 {
            k -= 1;
        }
        out[k] = p;
        k += 1;
    }
    let lower_len = k + 1;
    for &p in points.iter().rev().skip(1) {
        while k >= lower_len && cross(out[k - 2], out[k - 1], p) <= 0.0 {
            k -= 1;
        }
        out[k] = p;
        k += 1;
    }
    // The last point written repeats the first.
    Some(k - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interior_points_are_excluded() {
        let mut pts = [
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(4.0, 4.0),
            Point::new(0.0, 4.0),
            Point::new(2.0, 2.0),
            Point::new(1.0, 3.0),
        ];
        let mut out = [Point::new(0.0, 0.0); 12];
        let n = convex_hull(&mut pts, &mut out).unwrap();
        assert_eq!(n, 4);
        let hull = &out[..n];
        for corner in
            [Point::new(0.0, 0.0), Point::new(4.0, 0.0), Point::new(4.0, 4.0), Point::new(0.0, 4.0)]
        {
            assert!(hull.contains(&corner), "missing {corner:?}");
        }
    }

    #[test]
    fn hull_is_counter_clockwise() {
        let mut pts = [
            Point::new(0.0, 0.0),
            Point::new(3.0, 1.0),
            Point::new(2.0, 4.0),
            Point::new(-1.0, 2.0),
        ];
        let mut out = [Point::new(0.0, 0.0); 8];
        let n = convex_hull(&mut pts, &mut out).unwrap();
        let hull = &out[..n];
        // Every consecutive triple turns left.
        for i in 0..n {
            let a = hull[i];
            let b = hull[(i + 1) % n];
            let c = hull[(i + 2) % n];
            assert!(cross(a, b, c) > 0.0);
        }
    }

    #[test]
    fn collinear_points_collapse_to_endpoints() {
        let mut pts = [Point::new(0.0, 0.0), Point::new(1.0, 1.0), Point::new(2.0, 2.0)];
        let mut out = [Point::new(0.0, 0.0); 6];
        let n = convex_hull(&mut pts, &mut out).unwrap();
        assert_eq!(n, 2);
    }

    #[test]
    fn undersized_output_is_rejected() {
        let mut pts = [Point::new(0.0, 0.0), Point::new(1.0, 0.0), Point::new(0.0, 1.0)];
        let mut out = [Point::new(0.0, 0.0); 3];
        assert_eq!(convex_hull(&mut pts, &mut out), None);
    }
}
