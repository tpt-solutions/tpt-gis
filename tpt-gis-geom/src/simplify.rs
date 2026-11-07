//! Douglas-Peucker line simplification.
//!
//! Allocation-free: the caller supplies the keep-flags, the work stack, and the output
//! buffer, so it works in `no_std` builds.

use crate::distance::point_to_segment;
use crate::point::Point;

/// Simplifies a polyline with the Douglas-Peucker algorithm, writing the kept points to
/// `out` in order. The first and last points are always kept.
///
/// Every removed point lies within `tolerance` of the simplified line. Returns the
/// number of points written, or `None` if any buffer is too small: `keep` and `stack`
/// need at least `points.len()` entries and `out` likewise.
///
/// The work `stack` never holds more than `points.len()` entries, because each entry
/// corresponds to a distinct kept point.
///
/// # Examples
///
/// ```
/// use tpt_gis_geom::{simplify::simplify_into, Point};
///
/// let line = [
///     Point::new(0.0, 0.0), Point::new(1.0, 0.01), Point::new(2.0, -0.01),
///     Point::new(3.0, 0.0),
/// ];
/// let (mut keep, mut stack) = ([false; 4], [(0usize, 0usize); 4]);
/// let mut out = [Point::new(0.0, 0.0); 4];
/// let n = simplify_into(&line, 0.1, &mut keep, &mut stack, &mut out).unwrap();
/// assert_eq!(n, 2); // only the endpoints survive
/// ```
pub fn simplify_into(
    points: &[Point],
    tolerance: f64,
    keep: &mut [bool],
    stack: &mut [(usize, usize)],
    out: &mut [Point],
) -> Option<usize> {
    let n = points.len();
    if n < 3 {
        if out.len() < n {
            return None;
        }
        out[..n].copy_from_slice(points);
        return Some(n);
    }
    if keep.len() < n || stack.len() < n || out.len() < n {
        return None;
    }

    keep[..n].fill(false);
    keep[0] = true;
    keep[n - 1] = true;

    let mut sp = 0;
    stack[sp] = (0, n - 1);
    sp += 1;
    while sp > 0 {
        sp -= 1;
        let (start, end) = stack[sp];
        if end <= start + 1 {
            continue;
        }
        let mut worst_index = start + 1;
        let mut worst_distance = -1.0_f64;
        for (offset, &p) in points[start + 1..end].iter().enumerate() {
            let d = point_to_segment(p, points[start], points[end]);
            if d > worst_distance {
                worst_distance = d;
                worst_index = start + 1 + offset;
            }
        }
        if worst_distance > tolerance {
            keep[worst_index] = true;
            stack[sp] = (start, worst_index);
            stack[sp + 1] = (worst_index, end);
            sp += 2;
        }
    }

    let mut written = 0;
    for (i, &p) in points.iter().enumerate() {
        if keep[i] {
            out[written] = p;
            written += 1;
        }
    }
    Some(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs the simplifier with `N`-sized buffers; returns the output and count.
    fn run<const N: usize>(points: &[Point], tolerance: f64) -> ([Point; N], usize) {
        let mut keep = [false; N];
        let mut stack = [(0usize, 0usize); N];
        let mut out = [Point::new(0.0, 0.0); N];
        let written = simplify_into(points, tolerance, &mut keep, &mut stack, &mut out).unwrap();
        (out, written)
    }

    #[test]
    fn small_bumps_are_removed_and_endpoints_kept() {
        let line = [
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.01),
            Point::new(2.0, -0.01),
            Point::new(3.0, 0.0),
        ];
        let (out, n) = run::<4>(&line, 0.1);
        assert_eq!(n, 2);
        assert_eq!(out[..n], [Point::new(0.0, 0.0), Point::new(3.0, 0.0)]);
    }

    #[test]
    fn large_deviation_is_kept() {
        let line = [Point::new(0.0, 0.0), Point::new(1.0, 5.0), Point::new(2.0, 0.0)];
        assert_eq!(run::<3>(&line, 1.0).1, 3);
    }

    #[test]
    fn zero_tolerance_keeps_every_bend() {
        let line = [
            Point::new(0.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(2.0, 0.0),
            Point::new(3.0, 1.0),
        ];
        assert_eq!(run::<4>(&line, 0.0).1, 4);
    }

    #[test]
    fn short_buffers_are_rejected() {
        let line = [Point::new(0.0, 0.0), Point::new(1.0, 1.0), Point::new(2.0, 0.0)];
        let mut keep = [false; 2];
        let mut stack = [(0usize, 0usize); 3];
        let mut out = [Point::new(0.0, 0.0); 3];
        assert_eq!(simplify_into(&line, 0.1, &mut keep, &mut stack, &mut out), None);
    }
}
