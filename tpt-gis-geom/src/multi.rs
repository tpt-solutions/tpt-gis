//! Multi-part geometries and a borrowed geometry-collection type.
//!
//! Like the single-part primitives, these borrow their parts (`&[...]`) rather than
//! owning them, so they can be built over `const`/`static` data with no allocation.

use crate::bbox::Rect;
use crate::line_string::LineString;
use crate::point::Point;
use crate::polygon::Polygon;

/// Bounding box enclosing a set of optional boxes, or `None` if there are none.
fn union_all<I: Iterator<Item = Option<Rect>>>(boxes: I) -> Option<Rect> {
    boxes.flatten().reduce(|a, b| a.union(&b))
}

/// A set of points (an OGC `MultiPoint`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MultiPoint<'a> {
    /// The member points.
    pub points: &'a [Point],
}

impl MultiPoint<'_> {
    /// The bounding box of all member points, or `None` if there are none.
    #[must_use]
    pub fn bbox(&self) -> Option<Rect> {
        Rect::from_points(self.points)
    }
}

/// A set of line strings (an OGC `MultiLineString`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MultiLineString<'a> {
    /// The member line strings.
    pub lines: &'a [LineString<'a>],
}

impl MultiLineString<'_> {
    /// The bounding box of all member lines, or `None` if there are none.
    #[must_use]
    pub fn bbox(&self) -> Option<Rect> {
        union_all(self.lines.iter().map(LineString::bbox))
    }

    /// The total planar length of all member lines.
    #[must_use]
    pub fn length(&self) -> f64 {
        self.lines.iter().map(LineString::length).sum()
    }
}

/// A set of polygons (an OGC `MultiPolygon`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MultiPolygon<'a> {
    /// The member polygons.
    pub polygons: &'a [Polygon<'a>],
}

impl MultiPolygon<'_> {
    /// The bounding box of all member polygons, or `None` if there are none.
    #[must_use]
    pub fn bbox(&self) -> Option<Rect> {
        union_all(self.polygons.iter().map(Polygon::bbox))
    }

    /// The total planar area of all member polygons.
    ///
    /// Member polygons are assumed not to overlap (as OGC requires of valid
    /// multipolygons); overlapping members are double-counted.
    #[must_use]
    pub fn area(&self) -> f64 {
        self.polygons.iter().map(Polygon::area).sum()
    }

    /// Returns `true` if `point` lies inside any member polygon.
    #[must_use]
    pub fn contains_point(&self, point: Point) -> bool {
        self.polygons.iter().any(|p| p.contains_point(point))
    }
}

/// Any geometry, including nested collections, by reference.
///
/// This is the borrowed counterpart of the `tpt-gis-io` owned `Geometry` enum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GeometryRef<'a> {
    /// A single point.
    Point(Point),
    /// A single line string.
    LineString(LineString<'a>),
    /// A single polygon.
    Polygon(Polygon<'a>),
    /// A set of points.
    MultiPoint(MultiPoint<'a>),
    /// A set of line strings.
    MultiLineString(MultiLineString<'a>),
    /// A set of polygons.
    MultiPolygon(MultiPolygon<'a>),
    /// A heterogeneous collection of geometries.
    Collection(&'a [GeometryRef<'a>]),
}

impl GeometryRef<'_> {
    /// The bounding box of the geometry, or `None` if it has no coordinates.
    #[must_use]
    pub fn bbox(&self) -> Option<Rect> {
        match self {
            GeometryRef::Point(p) => Some(Rect::new(*p, *p)),
            GeometryRef::LineString(l) => l.bbox(),
            GeometryRef::Polygon(p) => p.bbox(),
            GeometryRef::MultiPoint(m) => m.bbox(),
            GeometryRef::MultiLineString(m) => m.bbox(),
            GeometryRef::MultiPolygon(m) => m.bbox(),
            GeometryRef::Collection(items) => union_all(items.iter().map(GeometryRef::bbox)),
        }
    }

    /// Returns `true` if `point` lies inside a polygonal part of the geometry (a point
    /// or line never "contains" a point in this sense, so they return `false`).
    #[must_use]
    pub fn contains_point(&self, point: Point) -> bool {
        match self {
            GeometryRef::Polygon(p) => p.contains_point(point),
            GeometryRef::MultiPolygon(m) => m.contains_point(point),
            GeometryRef::Collection(items) => items.iter().any(|g| g.contains_point(point)),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::float_ext::Fabs as _;

    const SQ_A: [Point; 5] = [
        Point::new(0.0, 0.0),
        Point::new(2.0, 0.0),
        Point::new(2.0, 2.0),
        Point::new(0.0, 2.0),
        Point::new(0.0, 0.0),
    ];
    const SQ_B: [Point; 5] = [
        Point::new(10.0, 10.0),
        Point::new(13.0, 10.0),
        Point::new(13.0, 13.0),
        Point::new(10.0, 13.0),
        Point::new(10.0, 10.0),
    ];

    #[test]
    fn multipolygon_area_and_containment() {
        let polys = [Polygon::from_exterior(&SQ_A), Polygon::from_exterior(&SQ_B)];
        let multi = MultiPolygon { polygons: &polys };
        assert!((multi.area() - (4.0 + 9.0)).fabs() < 1e-9);
        assert!(multi.contains_point(Point::new(1.0, 1.0)));
        assert!(multi.contains_point(Point::new(11.0, 11.0)));
        assert!(!multi.contains_point(Point::new(5.0, 5.0)));
    }

    #[test]
    fn multipolygon_bbox_spans_members() {
        let polys = [Polygon::from_exterior(&SQ_A), Polygon::from_exterior(&SQ_B)];
        let bbox = MultiPolygon { polygons: &polys }.bbox().unwrap();
        assert_eq!(bbox.min, Point::new(0.0, 0.0));
        assert_eq!(bbox.max, Point::new(13.0, 13.0));
    }

    #[test]
    fn empty_multi_has_no_bbox() {
        let multi = MultiPolygon { polygons: &[] };
        assert_eq!(multi.bbox(), None);
        assert_eq!(multi.area(), 0.0);
    }

    #[test]
    fn multilinestring_length_sums_members() {
        let a = [Point::new(0.0, 0.0), Point::new(3.0, 4.0)]; // 5
        let b = [Point::new(0.0, 0.0), Point::new(0.0, 2.0)]; // 2
        let lines = [LineString::new(&a), LineString::new(&b)];
        assert!((MultiLineString { lines: &lines }.length() - 7.0).fabs() < 1e-9);
    }

    #[test]
    fn collection_nests_and_reports_containment() {
        let poly = Polygon::from_exterior(&SQ_A);
        let inner = [GeometryRef::Polygon(poly), GeometryRef::Point(Point::new(20.0, 20.0))];
        let outer = [GeometryRef::Collection(&inner)];
        let g = GeometryRef::Collection(&outer);
        assert!(g.contains_point(Point::new(1.0, 1.0)));
        assert!(!g.contains_point(Point::new(5.0, 5.0)));
        let bbox = g.bbox().unwrap();
        assert_eq!(bbox.max, Point::new(20.0, 20.0));
    }
}
