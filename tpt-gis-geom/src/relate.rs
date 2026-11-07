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
use crate::polygon::Polygon;
use crate::predicates::{polygon_contains_polygon, polygons_intersect};

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
