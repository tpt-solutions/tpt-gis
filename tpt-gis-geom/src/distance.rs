//! Planar distance calculations between points, segments, and line strings.
//!
//! These are Cartesian (planar) distances in the units of the coordinate space —
//! for geodesic (ellipsoidal) distance between longitude/latitude points, see
//! `tpt-gis-core`'s `geodesic` module instead.

use libm::sqrt;

use crate::line_string::LineString;
use crate::point::Point;

/// Euclidean distance between two points.
#[must_use]
pub fn point_to_point(a: Point, b: Point) -> f64 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    sqrt(dx * dx + dy * dy)
}

/// The point on segment `a`-`b` nearest to `point`.
#[must_use]
pub fn closest_point_on_segment(point: Point, a: Point, b: Point) -> Point {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len_sq = dx * dx + dy * dy;

    if len_sq < f64::EPSILON {
        return a;
    }

    let t = ((point.x - a.x) * dx + (point.y - a.y) * dy) / len_sq;
    let t_clamped = t.clamp(0.0, 1.0);
    Point::new(a.x + t_clamped * dx, a.y + t_clamped * dy)
}

/// Shortest distance from `point` to the segment `a`-`b`.
#[must_use]
pub fn point_to_segment(point: Point, a: Point, b: Point) -> f64 {
    point_to_point(point, closest_point_on_segment(point, a, b))
}

/// Shortest distance from `point` to any segment of `line`.
///
/// Returns `0.0` for an empty line string (there is nothing to measure to).
#[must_use]
pub fn point_to_line_string(point: Point, line: &LineString) -> f64 {
    match line.points.len() {
        0 => 0.0,
        1 => point_to_point(point, line.points[0]),
        _ => line
            .segments()
            .map(|(a, b)| point_to_segment(point, a, b))
            .fold(f64::INFINITY, f64::min),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closest_point_on_segment_is_perpendicular_foot() {
        let closest =
            closest_point_on_segment(Point::new(5.0, 3.0), Point::new(0.0, 0.0), Point::new(10.0, 0.0));
        assert_eq!(closest, Point::new(5.0, 0.0));
    }

    #[test]
    fn closest_point_on_segment_clamps_to_endpoint() {
        let closest = closest_point_on_segment(
            Point::new(20.0, 5.0),
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
        );
        assert_eq!(closest, Point::new(10.0, 0.0));
    }

    #[test]
    fn point_to_point_pythagorean() {
        let d = point_to_point(Point::new(0.0, 0.0), Point::new(3.0, 4.0));
        assert!((d - 5.0).abs() < 1e-9);
    }

    #[test]
    fn point_to_segment_perpendicular_distance() {
        let d =
            point_to_segment(Point::new(0.0, 5.0), Point::new(-10.0, 0.0), Point::new(10.0, 0.0));
        assert!((d - 5.0).abs() < 1e-9);
    }

    #[test]
    fn point_to_segment_clamps_past_endpoint() {
        let d =
            point_to_segment(Point::new(20.0, 0.0), Point::new(0.0, 0.0), Point::new(10.0, 0.0));
        assert!((d - 10.0).abs() < 1e-9);
    }

    #[test]
    fn point_to_line_string_finds_nearest_segment() {
        let pts = [Point::new(0.0, 0.0), Point::new(10.0, 0.0), Point::new(10.0, 10.0)];
        let line = LineString::new(&pts);
        let d = point_to_line_string(Point::new(15.0, 5.0), &line);
        assert!((d - 5.0).abs() < 1e-9);
    }
}
