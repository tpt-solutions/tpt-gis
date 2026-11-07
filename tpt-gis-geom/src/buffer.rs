//! Buffers: the set of points within a distance of a geometry, with round joins and
//! round caps. Requires the `alloc` feature.
//!
//! The buffer is built from pieces: a rectangle per segment (the segment swept sideways
//! by the distance), and a regular polygon per vertex (the round join or cap). The
//! pieces are merged with boolean union from [`crate::clip`]. Curves are approximated
//! by `segments` sides per full circle, so the result is slightly inside the true
//! buffer; the error shrinks as `segments` grows (64 keeps the area within 0.2%).
//!
//! Supported inputs: points, lines, and polygons without holes. Polygons with holes
//! and negative distances (erosion) return `None`.

use alloc::vec::Vec;

use libm::{cos, sin, sqrt};

use crate::clip::{self, OwnedPolygon};
use crate::point::Point;
use crate::polygon::Polygon;

/// Smallest accepted `segments` value; a circle needs at least three sides.
const MIN_SEGMENTS: usize = 4;

/// The buffer of a point: a regular polygon of radius `distance`.
#[must_use]
pub fn buffer_point(p: Point, distance: f64, segments: usize) -> Vec<OwnedPolygon> {
    if distance.is_nan() || distance <= 0.0 {
        return Vec::new();
    }
    vec_of(disk(p, distance, segments))
}

/// The buffer of a polyline: every segment is swept by `distance`, with round caps at
/// the ends and round joins at interior vertices.
///
/// Returns an empty vector for a non-positive distance, and a disk for a line with a
/// single distinct point.
#[must_use]
pub fn buffer_line(points: &[Point], distance: f64, segments: usize) -> Vec<OwnedPolygon> {
    if distance.is_nan() || distance <= 0.0 {
        return Vec::new();
    }
    let mut pieces: Vec<Vec<Point>> = Vec::new();
    for w in points.windows(2) {
        if let Some(rect) = segment_rect(w[0], w[1], distance) {
            pieces.push(rect);
        }
    }
    for &p in points {
        pieces.push(disk(p, distance, segments));
    }
    merge_all(pieces)
}

/// The buffer of a hole-free polygon: the polygon itself, plus its edges swept outward
/// and round joins at its vertices. Returns `None` if the polygon has holes or the
/// distance is negative (erosion is not supported). A zero distance returns the polygon
/// unchanged.
#[must_use]
pub fn buffer_polygon(
    polygon: &Polygon,
    distance: f64,
    segments: usize,
) -> Option<Vec<OwnedPolygon>> {
    if !polygon.interiors.is_empty() || distance < 0.0 {
        return None;
    }
    let ring = polygon.exterior;
    if distance == 0.0 {
        return Some(vec_of(ring.to_vec()));
    }
    let mut pieces: Vec<Vec<Point>> = alloc::vec![ring.to_vec()];
    let n = ring.len();
    for i in 0..n {
        if let Some(rect) = segment_rect(ring[i], ring[(i + 1) % n], distance) {
            pieces.push(rect);
        }
    }
    for &p in ring {
        pieces.push(disk(p, distance, segments));
    }
    Some(merge_all(pieces))
}

/// The regular `segments`-gon of radius `r` centred on `c`, as a ring.
fn disk(c: Point, r: f64, segments: usize) -> Vec<Point> {
    let n = segments.max(MIN_SEGMENTS);
    let mut ring = Vec::with_capacity(n + 1);
    for k in 0..n {
        let theta = core::f64::consts::TAU * k as f64 / n as f64;
        ring.push(Point::new(c.x + r * cos(theta), c.y + r * sin(theta)));
    }
    ring.push(ring[0]);
    ring
}

/// The rectangle swept by segment `a → b` sideways by `r` on both sides, or `None` for
/// a zero-length segment.
fn segment_rect(a: Point, b: Point, r: f64) -> Option<Vec<Point>> {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len = sqrt(dx * dx + dy * dy);
    if len == 0.0 {
        return None;
    }
    // Unit normal, scaled to the distance.
    let (nx, ny) = (-dy / len * r, dx / len * r);
    Some(alloc::vec![
        Point::new(a.x + nx, a.y + ny),
        Point::new(b.x + nx, b.y + ny),
        Point::new(b.x - nx, b.y - ny),
        Point::new(a.x - nx, a.y - ny),
        Point::new(a.x + nx, a.y + ny),
    ])
}

fn vec_of(ring: Vec<Point>) -> Vec<OwnedPolygon> {
    alloc::vec![OwnedPolygon { rings: alloc::vec![ring] }]
}

/// Runs `f` on a borrowed view of an owned polygon.
fn with_polygon<R>(poly: &OwnedPolygon, f: impl FnOnce(&Polygon) -> R) -> R {
    let holes: Vec<&[Point]> = poly.rings[1..].iter().map(Vec::as_slice).collect();
    let view = Polygon::new(&poly.rings[0], &holes);
    f(&view)
}

/// Unions a set of pieces: repeatedly merges any two components whose union is a single
/// polygon, until no pair overlaps.
fn merge_all(pieces: Vec<Vec<Point>>) -> Vec<OwnedPolygon> {
    let mut parts: Vec<OwnedPolygon> =
        pieces.into_iter().map(|ring| OwnedPolygon { rings: alloc::vec![ring] }).collect();
    loop {
        let mut merged = None;
        'search: for i in 0..parts.len() {
            for j in (i + 1)..parts.len() {
                let union =
                    with_polygon(&parts[i], |a| with_polygon(&parts[j], |b| clip::union(a, b)));
                if union.len() == 1 {
                    merged = Some((i, j, union));
                    break 'search;
                }
            }
        }
        match merged {
            Some((i, j, union)) => {
                // Remove the later index first so the earlier one stays valid.
                parts.remove(j);
                parts.remove(i);
                parts.extend(union);
            }
            None => return parts,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn total_area(parts: &[OwnedPolygon]) -> f64 {
        parts.iter().map(OwnedPolygon::area).sum()
    }

    fn rel_err(actual: f64, expected: f64) -> f64 {
        ((actual - expected) / expected).abs()
    }

    #[test]
    fn point_buffer_is_a_disk() {
        let parts = buffer_point(Point::new(3.0, 4.0), 1.0, 64);
        assert_eq!(parts.len(), 1);
        assert!(rel_err(total_area(&parts), core::f64::consts::PI) < 2e-3);
    }

    /// Round caps: a 10-long segment buffered by 1 is a 10×2 rectangle plus a disk.
    #[test]
    fn line_buffer_area_matches_stadium() {
        let line = [Point::new(0.0, 0.0), Point::new(10.0, 0.0)];
        let parts = buffer_line(&line, 1.0, 64);
        assert_eq!(parts.len(), 1, "{parts:?}");
        let expected = 20.0 + core::f64::consts::PI;
        assert!(rel_err(total_area(&parts), expected) < 1e-2, "{}", total_area(&parts));
    }

    /// Round joins: an L of two 10-long legs buffered by 1 is two stadia overlapping at the
    /// corner, which is 2·(20 + π) minus the overlap of the two corner disks' halves.
    #[test]
    fn polyline_buffer_is_connected_across_joins() {
        let l = [Point::new(0.0, 0.0), Point::new(10.0, 0.0), Point::new(10.0, 10.0)];
        let parts = buffer_line(&l, 1.0, 64);
        assert_eq!(parts.len(), 1);
        // Two 10×2 rectangles, a corner square-ish join of area π (quarter disk ×4 total
        // beyond the rectangles' corners), and two end caps of π/2 each.
        let expected = 40.0 + core::f64::consts::PI;
        assert!(rel_err(total_area(&parts), expected) < 2e-2, "{}", total_area(&parts));
    }

    /// Square of side 10 buffered by 1: the square, four edge strips, four quarter disks.
    #[test]
    fn square_buffer_area() {
        let ring = [
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 10.0),
            Point::new(0.0, 10.0),
            Point::new(0.0, 0.0),
        ];
        let poly = Polygon::from_exterior(&ring);
        let parts = buffer_polygon(&poly, 1.0, 64).unwrap();
        assert_eq!(parts.len(), 1);
        let expected = 100.0 + 40.0 + core::f64::consts::PI;
        assert!(rel_err(total_area(&parts), expected) < 1e-2, "{}", total_area(&parts));
    }

    #[test]
    fn zero_distance_returns_the_polygon() {
        let ring = [
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(4.0, 4.0),
            Point::new(0.0, 4.0),
            Point::new(0.0, 0.0),
        ];
        let poly = Polygon::from_exterior(&ring);
        let parts = buffer_polygon(&poly, 0.0, 8).unwrap();
        assert!((total_area(&parts) - 16.0).abs() < 1e-9);
    }

    #[test]
    fn unsupported_inputs_are_refused() {
        let ring = [
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(4.0, 4.0),
            Point::new(0.0, 0.0),
        ];
        let hole = [
            Point::new(1.0, 1.0),
            Point::new(2.0, 1.0),
            Point::new(2.0, 2.0),
            Point::new(1.0, 1.0),
        ];
        let holes: [&[Point]; 1] = [&hole];
        assert!(buffer_polygon(&Polygon::new(&ring, &holes), 1.0, 8).is_none());
        assert!(buffer_polygon(&Polygon::from_exterior(&ring), -1.0, 8).is_none());
        assert!(buffer_point(Point::new(0.0, 0.0), 0.0, 8).is_empty());
    }
}
