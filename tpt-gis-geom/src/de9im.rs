//! The full DE-9IM intersection matrix between two geometries (OGC 06-103r4, section 6.1.15).
//!
//! The matrix has nine cells: the dimension of the intersection of each of the interior,
//! boundary and exterior of the first geometry with each of those of the second. Requires
//! the `alloc` feature.
//!
//! Supported inputs are points, multi-points, line strings, multi-line strings, polygons and
//! multi-polygons. A [`GeometryRef::Collection`] has no single DE-9IM matrix, so [`relate`]
//! returns `None` for it.
//!
//! Boundaries follow the OGC rules. A line string's boundary is its two end points, unless it
//! is closed, when it is empty. A multi-line string's boundary is the end points that occur an
//! odd number of times. A polygon's boundary is its rings. A point has no boundary.
//!
//! Known limitation: the parts of a multi-polygon are assumed not to overlap. A point shared by
//! the edges of two touching parts counts as on the boundary, where the OGC union would call it
//! interior.
//!
//! Method: each curve (a line segment or a ring edge) is split wherever it meets the other
//! geometry's curves, and each piece's midpoint is located in the other geometry. Every vertex
//! and split point is located in both geometries too. Two-dimensional cells come from areas.

use alloc::vec::Vec;
use core::fmt;

use crate::clip;
use crate::distance;
use crate::line_relate::{segment_contact, sub, Contact};
use crate::multi::GeometryRef;
use crate::point::Point;
use crate::polygon::Polygon;
use crate::predicates::{on_polygon_boundary, ring_edges, rings, strictly_inside, tolerance_for};

/// Relative tolerance for "no area" when comparing against the combined area.
const AREA_TOLERANCE: f64 = 1e-9;

/// Where a point lies relative to a geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Location {
    /// In the geometry's interior.
    Interior,
    /// On the geometry's boundary.
    Boundary,
    /// Outside the geometry.
    Exterior,
}

impl Location {
    fn index(self) -> usize {
        match self {
            Location::Interior => 0,
            Location::Boundary => 1,
            Location::Exterior => 2,
        }
    }
}

/// The DE-9IM intersection matrix of two geometries.
///
/// Each cell is the dimension of an intersection: `-1` for empty, `0`, `1` or `2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntersectionMatrix {
    cells: [i8; 9],
}

impl IntersectionMatrix {
    /// The dimension of the intersection of the first geometry's `a` set with the second
    /// geometry's `b` set: `-1` if empty, otherwise `0`, `1` or `2`.
    #[must_use]
    pub fn get(&self, a: Location, b: Location) -> i8 {
        self.cells[a.index() * 3 + b.index()]
    }

    /// Whether the matrix matches a nine-character DE-9IM pattern.
    ///
    /// `T` means non-empty (dimension 0, 1 or 2), `F` means empty, `*` matches anything, and
    /// `0`, `1` or `2` require that exact dimension. A pattern that is not nine such characters
    /// matches nothing.
    #[must_use]
    pub fn matches(&self, pattern: &str) -> bool {
        let bytes = pattern.as_bytes();
        bytes.len() == 9
            && bytes.iter().zip(self.cells).all(|(&p, cell)| match p {
                b'T' => cell >= 0,
                b'F' => cell == -1,
                b'*' => true,
                b'0'..=b'2' => i16::from(cell) == i16::from(p - b'0'),
                _ => false,
            })
    }
}

impl fmt::Display for IntersectionMatrix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for cell in self.cells {
            let c = match cell {
                -1 => 'F',
                0 => '0',
                1 => '1',
                _ => '2',
            };
            write!(f, "{c}")?;
        }
        Ok(())
    }
}

/// A geometry flattened into its points, line strings and polygons. A geometry has only one
/// of these kinds populated.
struct Shape<'a> {
    points: Vec<Point>,
    lines: Vec<&'a [Point]>,
    polygons: Vec<Polygon<'a>>,
}

impl<'a> Shape<'a> {
    fn from_ref(geometry: &GeometryRef<'a>) -> Option<Self> {
        let mut shape = Shape { points: Vec::new(), lines: Vec::new(), polygons: Vec::new() };
        match geometry {
            GeometryRef::Point(p) => shape.points.push(*p),
            GeometryRef::MultiPoint(m) => shape.points.extend_from_slice(m.points),
            GeometryRef::LineString(l) => shape.push_line(l.points),
            GeometryRef::MultiLineString(m) => {
                for line in m.lines {
                    shape.push_line(line.points);
                }
            }
            GeometryRef::Polygon(p) => shape.polygons.push(*p),
            GeometryRef::MultiPolygon(m) => shape.polygons.extend_from_slice(m.polygons),
            GeometryRef::Collection(_) => return None,
        }
        Some(shape)
    }

    /// Lines with fewer than two points have no extent and are ignored.
    fn push_line(&mut self, points: &'a [Point]) {
        if points.len() >= 2 {
            self.lines.push(points);
        }
    }

    /// Every segment of every line and ring, with the location of its interior in this shape.
    fn curves(&self) -> Vec<(Point, Point, Location)> {
        let mut out = Vec::new();
        for line in &self.lines {
            for w in line.windows(2) {
                if w[0] != w[1] {
                    out.push((w[0], w[1], Location::Interior));
                }
            }
        }
        for poly in &self.polygons {
            for ring in rings(poly) {
                for i in 0..ring.len() {
                    let (a, b) = ring_edges(ring, i);
                    if a != b {
                        out.push((a, b, Location::Boundary));
                    }
                }
            }
        }
        out
    }

    /// Every vertex and point of the shape.
    fn vertices(&self) -> Vec<Point> {
        let mut out = self.points.clone();
        for line in &self.lines {
            out.extend_from_slice(line);
        }
        for poly in &self.polygons {
            for ring in rings(poly) {
                out.extend_from_slice(ring);
            }
        }
        out
    }

    /// Where `p` lies relative to this shape.
    fn locate(&self, p: Point) -> Location {
        if !self.polygons.is_empty() {
            if self.polygons.iter().any(|poly| strictly_inside(poly, p)) {
                return Location::Interior;
            }
            if self.polygons.iter().any(|poly| on_polygon_boundary(poly, p)) {
                return Location::Boundary;
            }
            return Location::Exterior;
        }
        if !self.lines.is_empty() {
            let tol = tolerance_for(&[p]);
            let mut boundary_hits = 0usize;
            let mut on_line = false;
            for line in &self.lines {
                if !is_closed(line) {
                    let (first, last) = (line[0], line[line.len() - 1]);
                    boundary_hits += usize::from(same_point(p, first));
                    boundary_hits += usize::from(same_point(p, last));
                }
                on_line |=
                    line.windows(2).any(|w| distance::point_to_segment(p, w[0], w[1]) <= tol);
            }
            // The mod-2 rule: an end point shared by an even number of lines is interior.
            if boundary_hits % 2 == 1 {
                return Location::Boundary;
            }
            return if on_line { Location::Interior } else { Location::Exterior };
        }
        if self.points.iter().any(|&q| same_point(p, q)) {
            Location::Interior
        } else {
            Location::Exterior
        }
    }
}

fn is_closed(line: &[Point]) -> bool {
    line.len() >= 2 && same_point(line[0], line[line.len() - 1])
}

fn same_point(p: Point, q: Point) -> bool {
    distance::point_to_point(p, q) <= tolerance_for(&[p, q])
}

fn set(cells: &mut [i8; 9], a: Location, b: Location, dim: i8) {
    let index = a.index() * 3 + b.index();
    cells[index] = cells[index].max(dim);
}

/// Parameter of `x` along the segment `p → q`, from 0 at `p` to 1 at `q`.
fn param(p: Point, q: Point, x: Point) -> f64 {
    let d = sub(q, p);
    let len2 = d.0 * d.0 + d.1 * d.1;
    let v = sub(x, p);
    (v.0 * d.0 + v.1 * d.1) / len2
}

/// Splits the segment `p → q` at every contact with the segments in `others` and at every
/// point in `points` that lies on it, records the split points in `nodes`, and returns the
/// pieces between consecutive split points.
fn split_segment(
    p: Point,
    q: Point,
    others: &[(Point, Point, Location)],
    points: &[Point],
    nodes: &mut Vec<Point>,
) -> Vec<(Point, Point)> {
    let mut cuts: Vec<(f64, Point)> = Vec::new();
    cuts.push((0.0, p));
    cuts.push((1.0, q));
    // A point geometry is not a curve, so it cannot split a segment through contacts. Cut here
    // too, or a piece's midpoint could land exactly on the point and be misread as its interior.
    for &x in points {
        if distance::point_to_segment(x, p, q) <= tolerance_for(&[x, p, q]) {
            cuts.push((param(p, q, x), x));
        }
    }
    for &(u, v, _) in others {
        match segment_contact(p, q, u, v) {
            Contact::None => {}
            Contact::Point(x) => cuts.push((param(p, q, x), x)),
            Contact::Overlap(x1, x2) => {
                cuts.push((param(p, q, x1), x1));
                cuts.push((param(p, q, x2), x2));
            }
        }
    }
    cuts.sort_by(|a, b| a.0.total_cmp(&b.0));

    let tol = tolerance_for(&[p, q]);
    let mut kept: Vec<Point> = Vec::new();
    for (_, x) in cuts {
        if kept.last().is_none_or(|&last| distance::point_to_point(last, x) > tol) {
            kept.push(x);
        }
    }
    nodes.extend_from_slice(&kept);
    kept.windows(2).map(|w| (w[0], w[1])).collect()
}

/// The DE-9IM matrix of `a` and `b`, or `None` if either is a geometry collection.
///
/// An empty geometry has an empty interior, boundary and exterior intersection with everything,
/// so its row and column are `F` apart from the exterior cell.
#[must_use]
pub fn relate(a: &GeometryRef, b: &GeometryRef) -> Option<IntersectionMatrix> {
    let sa = Shape::from_ref(a)?;
    let sb = Shape::from_ref(b)?;
    let mut cells = [-1i8; 9];
    // Both geometries are bounded, so their exteriors always meet in an open region.
    set(&mut cells, Location::Exterior, Location::Exterior, 2);

    let ca = sa.curves();
    let cb = sb.curves();

    // Curve pieces: each piece lies wholly inside one location of the other geometry.
    let mut nodes: Vec<Point> = sa.vertices();
    nodes.extend(sb.vertices());
    for &(p, q, own) in &ca {
        for (x, y) in split_segment(p, q, &cb, &sb.points, &mut nodes) {
            let mid = Point::new((x.x + y.x) / 2.0, (x.y + y.y) / 2.0);
            set(&mut cells, own, sb.locate(mid), 1);
        }
    }
    for &(p, q, own) in &cb {
        for (x, y) in split_segment(p, q, &ca, &sa.points, &mut nodes) {
            let mid = Point::new((x.x + y.x) / 2.0, (x.y + y.y) / 2.0);
            set(&mut cells, sa.locate(mid), own, 1);
        }
    }

    // Points: every vertex and split point, located in both geometries.
    for &x in &nodes {
        set(&mut cells, sa.locate(x), sb.locate(x), 0);
    }

    // Areas: interiors meet in an open set, so a positive overlap is a two-dimensional cell.
    if !sa.polygons.is_empty() || !sb.polygons.is_empty() {
        let area_a: f64 = sa.polygons.iter().map(Polygon::area).sum();
        let area_b: f64 = sb.polygons.iter().map(Polygon::area).sum();
        let mut overlap = 0.0;
        for pa in &sa.polygons {
            for pb in &sb.polygons {
                overlap +=
                    clip::intersection(pa, pb).iter().map(clip::OwnedPolygon::area).sum::<f64>();
            }
        }
        let tol = AREA_TOLERANCE * (area_a + area_b);
        if overlap > tol {
            set(&mut cells, Location::Interior, Location::Interior, 2);
        }
        if !sa.polygons.is_empty() && area_a - overlap > tol {
            set(&mut cells, Location::Interior, Location::Exterior, 2);
        }
        if !sb.polygons.is_empty() && area_b - overlap > tol {
            set(&mut cells, Location::Exterior, Location::Interior, 2);
        }
    }

    Some(IntersectionMatrix { cells })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::line_string::LineString;
    use crate::multi::{MultiLineString, MultiPoint};
    use alloc::string::{String, ToString};

    fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> [Point; 5] {
        [
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
            Point::new(x0, y0),
        ]
    }

    fn poly(ring: &[Point]) -> GeometryRef<'_> {
        GeometryRef::Polygon(Polygon::from_exterior(ring))
    }

    fn matrix(a: &GeometryRef, b: &GeometryRef) -> String {
        relate(a, b).unwrap().to_string()
    }

    #[test]
    fn touching_squares_share_an_edge() {
        let (a, b) = (square(0.0, 0.0, 10.0, 10.0), square(10.0, 0.0, 20.0, 10.0));
        assert_eq!(matrix(&poly(&a), &poly(&b)), "FF2F11212");
    }

    #[test]
    fn overlapping_squares() {
        let (a, b) = (square(0.0, 0.0, 10.0, 10.0), square(5.0, 5.0, 15.0, 15.0));
        assert_eq!(matrix(&poly(&a), &poly(&b)), "212101212");
    }

    #[test]
    fn nested_squares_are_within() {
        let (a, b) = (square(2.0, 2.0, 4.0, 4.0), square(0.0, 0.0, 10.0, 10.0));
        let m = relate(&poly(&a), &poly(&b)).unwrap();
        assert_eq!(m.to_string(), "2FF1FF212");
        assert!(m.matches("T*F**F***"), "within");
    }

    #[test]
    fn identical_squares_are_equal() {
        let a = square(0.0, 0.0, 10.0, 10.0);
        let m = relate(&poly(&a), &poly(&a)).unwrap();
        assert_eq!(m.to_string(), "2FFF1FFF2");
        assert!(m.matches("T*F**FFF*"), "equals");
    }

    #[test]
    fn point_outside_and_inside_a_square() {
        let a = square(0.0, 0.0, 10.0, 10.0);
        let outside = GeometryRef::Point(Point::new(20.0, 20.0));
        let inside = GeometryRef::Point(Point::new(5.0, 5.0));
        assert_eq!(matrix(&outside, &poly(&a)), "FF0FFF212");
        assert_eq!(matrix(&inside, &poly(&a)), "0FFFFF212");
    }

    #[test]
    fn point_at_the_midpoint_of_an_edge_is_on_the_boundary() {
        // (5, 0) is the midpoint of the bottom edge: the edge must split there, or its midpoint
        // would be misread as the point's interior.
        let a = square(0.0, 0.0, 10.0, 10.0);
        let on_edge = GeometryRef::Point(Point::new(5.0, 0.0));
        assert_eq!(matrix(&on_edge, &poly(&a)), "F0FFFF212");
    }

    #[test]
    fn line_crossing_a_square() {
        let a = square(0.0, 0.0, 10.0, 10.0);
        let pts = [Point::new(-5.0, 5.0), Point::new(15.0, 5.0)];
        let line = GeometryRef::LineString(LineString::new(&pts));
        // Crossing: the line's interior meets the polygon interior and exterior.
        assert_eq!(matrix(&line, &poly(&a)), "101FF0212");
    }

    #[test]
    fn closed_line_has_no_boundary() {
        let pts = square(0.0, 0.0, 4.0, 4.0);
        let line = GeometryRef::LineString(LineString::new(&pts));
        let far = square(20.0, 20.0, 24.0, 24.0);
        // Disjoint from the polygon: the closed line has no boundary, and its interior lies
        // in the polygon's exterior.
        let m = relate(&line, &poly(&far)).unwrap();
        assert_eq!(m.get(Location::Boundary, Location::Exterior), -1);
        assert_eq!(m.get(Location::Boundary, Location::Interior), -1);
        assert_eq!(m.get(Location::Interior, Location::Exterior), 1);
    }

    #[test]
    fn multi_line_boundary_uses_the_mod_two_rule() {
        // Two lines meeting end to end at (2, 0): that point is interior to the union.
        let first = [Point::new(0.0, 0.0), Point::new(2.0, 0.0)];
        let second = [Point::new(2.0, 0.0), Point::new(4.0, 0.0)];
        let lines = [LineString::new(&first), LineString::new(&second)];
        let multi = GeometryRef::MultiLineString(MultiLineString { lines: &lines });
        let joint = GeometryRef::Point(Point::new(2.0, 0.0));
        assert_eq!(matrix(&joint, &multi), "0FFFFF102");
    }

    #[test]
    fn multi_point_against_polygon() {
        let a = square(0.0, 0.0, 10.0, 10.0);
        let pts = [Point::new(5.0, 5.0), Point::new(50.0, 50.0)];
        let multi = GeometryRef::MultiPoint(MultiPoint { points: &pts });
        let m = relate(&multi, &poly(&a)).unwrap();
        assert_eq!(m.get(Location::Interior, Location::Interior), 0);
        assert_eq!(m.get(Location::Interior, Location::Exterior), 0);
    }

    #[test]
    fn collections_have_no_matrix() {
        let a = square(0.0, 0.0, 1.0, 1.0);
        let items = [poly(&a)];
        assert!(relate(&GeometryRef::Collection(&items), &poly(&a)).is_none());
    }

    #[test]
    fn pattern_matching() {
        let (a, b) = (square(0.0, 0.0, 10.0, 10.0), square(10.0, 0.0, 20.0, 10.0));
        let m = relate(&poly(&a), &poly(&b)).unwrap();
        assert!(m.matches("FF2F11212"));
        assert!(m.matches("FF*F*****"));
        assert!(m.matches("FFTF1TTTT"));
        assert!(!m.matches("FF2F11211"));
        assert!(!m.matches("FF2F1121"), "too short");
        assert!(!m.matches("FF2F1121X"), "unknown character");
    }
}
