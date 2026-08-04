//! A shared, owned geometry representation used by every format reader/writer in
//! this crate (GeoJSON, WKT, WKB, Shapefile).
//!
//! `tpt-gis-geom`'s `Point`/`LineString`/`Polygon` *borrow* their coordinate data
//! (`&[Point]`) so they can be built over `const`/`static` data with zero
//! allocation — a hard requirement for its embedded use cases (see that crate's
//! docs). Parsed file data has no such `'static` backing storage, so this crate
//! needs its own *owned* (`Vec`-based) geometry types instead. [`Point`] itself
//! (plain `{x, y}` data, no lifetime) is reused directly from `tpt-gis-geom`.

pub use tpt_gis_geom::{Point, Rect};

/// A polygon: a list of rings, where the first ring is the exterior boundary and
/// any remaining rings are interior boundaries (holes).
#[derive(Debug, Clone, PartialEq)]
pub struct Polygon {
    /// `rings[0]` is the exterior ring; `rings[1..]` are interior rings (holes).
    /// Each ring is closed (first point equals last), per the OGC convention.
    pub rings: Vec<Vec<Point>>,
}

impl Polygon {
    /// Constructs a polygon with no holes.
    #[must_use]
    pub fn from_exterior(exterior: Vec<Point>) -> Self {
        Self { rings: vec![exterior] }
    }

    /// The exterior (outer boundary) ring.
    ///
    /// # Panics
    /// Panics if `rings` is empty — a valid `Polygon` always has at least one ring.
    #[must_use]
    pub fn exterior(&self) -> &[Point] {
        &self.rings[0]
    }

    /// Interior (hole) rings, if any.
    #[must_use]
    pub fn interiors(&self) -> &[Vec<Point>] {
        &self.rings[1..]
    }

    /// Borrows this polygon as a `tpt_gis_geom::Polygon` for use with that crate's
    /// topological predicates (`contains_point`, `nearest_boundary_point`, etc).
    ///
    /// `tpt_gis_geom::Polygon` represents interior rings as `&[&[Point]]`, which
    /// requires storage for the slice-of-slices to live somewhere; `interior_scratch`
    /// is that storage, owned by the caller so its lifetime can outlive the returned
    /// borrow. Pass an empty, reusable `Vec` — it's cleared and repopulated here.
    pub fn as_geom<'a>(
        &'a self,
        interior_scratch: &'a mut Vec<&'a [Point]>,
    ) -> tpt_gis_geom::Polygon<'a> {
        interior_scratch.clear();
        interior_scratch.extend(self.interiors().iter().map(Vec::as_slice));
        tpt_gis_geom::Polygon::new(self.exterior(), interior_scratch)
    }

    /// Computes the bounding box (envelope) of this polygon's exterior ring.
    #[must_use]
    pub fn bbox(&self) -> Option<Rect> {
        Rect::from_points(self.exterior())
    }
}

/// An OGC Simple Features geometry, owned (backed by `Vec`s rather than borrowed
/// slices) so it can represent data parsed from a file or network stream.
#[derive(Debug, Clone, PartialEq)]
pub enum Geometry {
    /// A single coordinate.
    Point(Point),
    /// An ordered, open path.
    LineString(Vec<Point>),
    /// An exterior ring with zero or more interior rings (holes).
    Polygon(Polygon),
    /// A collection of points.
    MultiPoint(Vec<Point>),
    /// A collection of line strings.
    MultiLineString(Vec<Vec<Point>>),
    /// A collection of polygons.
    MultiPolygon(Vec<Polygon>),
    /// A heterogeneous collection of geometries.
    GeometryCollection(Vec<Geometry>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polygon_exterior_and_interiors() {
        let exterior = vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0), Point::new(0.0, 0.0)];
        let hole = vec![Point::new(1.0, 1.0), Point::new(2.0, 1.0), Point::new(1.0, 1.0)];
        let polygon = Polygon { rings: vec![exterior.clone(), hole.clone()] };

        assert_eq!(polygon.exterior(), exterior.as_slice());
        assert_eq!(polygon.interiors(), &[hole]);
    }

    #[test]
    fn polygon_as_geom_contains_point() {
        let exterior = vec![
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 10.0),
            Point::new(0.0, 10.0),
            Point::new(0.0, 0.0),
        ];
        let polygon = Polygon::from_exterior(exterior);

        let mut scratch = Vec::new();
        let geom = polygon.as_geom(&mut scratch);
        assert!(geom.contains_point(Point::new(5.0, 5.0)));
        assert!(!geom.contains_point(Point::new(15.0, 5.0)));
    }
}
