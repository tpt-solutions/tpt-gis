//! Planar area and centroid of polygons.
//!
//! Both are computed with the shoelace formula, so they are exact up to floating-point
//! rounding for simple (non-self-intersecting) rings. Area is always non-negative
//! regardless of ring orientation; holes are subtracted.

use crate::point::Point;
use crate::polygon::Polygon;
use crate::predicates::ring_edges;

/// Signed planar area of a closed ring by the shoelace formula: positive for
/// counter-clockwise rings, negative for clockwise ones, `0.0` for fewer than 3 points.
#[must_use]
pub fn ring_signed_area(ring: &[Point]) -> f64 {
    if ring.len() < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    for i in 0..ring.len() {
        let (a, b) = ring_edges(ring, i);
        sum += a.x * b.y - b.x * a.y;
    }
    sum / 2.0
}

/// The area-weighted centroid of a single ring, together with its absolute area.
/// Returns `None` for rings with zero area (their centroid is undefined).
fn ring_centroid(ring: &[Point]) -> Option<(Point, f64)> {
    let signed = ring_signed_area(ring);
    if signed == 0.0 {
        return None;
    }
    let mut cx = 0.0;
    let mut cy = 0.0;
    for i in 0..ring.len() {
        let (a, b) = ring_edges(ring, i);
        let cross = a.x * b.y - b.x * a.y;
        cx += (a.x + b.x) * cross;
        cy += (a.y + b.y) * cross;
    }
    let factor = 1.0 / (6.0 * signed);
    Some((Point::new(cx * factor, cy * factor), signed.abs()))
}

impl Polygon<'_> {
    /// The planar area of the polygon: the exterior ring's area minus the area of each
    /// hole. Always non-negative.
    #[must_use]
    pub fn area(&self) -> f64 {
        let exterior = ring_signed_area(self.exterior).abs();
        let holes: f64 = self.interiors.iter().map(|h| ring_signed_area(h).abs()).sum();
        exterior - holes
    }

    /// The area-weighted centroid of the polygon (holes removed), or `None` if the
    /// polygon has zero area.
    #[must_use]
    pub fn centroid(&self) -> Option<Point> {
        let (ext_c, ext_a) = ring_centroid(self.exterior)?;
        let mut sum_x = ext_c.x * ext_a;
        let mut sum_y = ext_c.y * ext_a;
        let mut total = ext_a;
        for hole in self.interiors.iter() {
            if let Some((c, a)) = ring_centroid(hole) {
                sum_x -= c.x * a;
                sum_y -= c.y * a;
                total -= a;
            }
        }
        if total <= 0.0 {
            return None;
        }
        Some(Point::new(sum_x / total, sum_y / total))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQUARE: [Point; 5] = [
        Point::new(0.0, 0.0),
        Point::new(10.0, 0.0),
        Point::new(10.0, 10.0),
        Point::new(0.0, 10.0),
        Point::new(0.0, 0.0),
    ];

    #[test]
    fn square_area_and_centroid() {
        let poly = Polygon::from_exterior(&SQUARE);
        assert!((poly.area() - 100.0).abs() < 1e-9);
        let c = poly.centroid().unwrap();
        assert!((c.x - 5.0).abs() < 1e-9 && (c.y - 5.0).abs() < 1e-9);
    }

    #[test]
    fn area_ignores_orientation() {
        let cw = [
            Point::new(0.0, 0.0),
            Point::new(0.0, 10.0),
            Point::new(10.0, 10.0),
            Point::new(10.0, 0.0),
            Point::new(0.0, 0.0),
        ];
        assert!((Polygon::from_exterior(&cw).area() - 100.0).abs() < 1e-9);
        assert!(ring_signed_area(&cw) < 0.0);
    }

    #[test]
    fn hole_is_subtracted_and_centroid_stays_central() {
        let hole = [
            Point::new(4.0, 4.0),
            Point::new(6.0, 4.0),
            Point::new(6.0, 6.0),
            Point::new(4.0, 6.0),
            Point::new(4.0, 4.0),
        ];
        let holes: [&[Point]; 1] = [&hole];
        let poly = Polygon::new(&SQUARE, &holes);
        assert!((poly.area() - 96.0).abs() < 1e-9);
        let c = poly.centroid().unwrap();
        assert!((c.x - 5.0).abs() < 1e-9 && (c.y - 5.0).abs() < 1e-9);
    }

    #[test]
    fn off_centre_hole_moves_centroid_away_from_it() {
        // Hole in the top-left quadrant pulls the centroid toward the bottom-right.
        let hole = [
            Point::new(1.0, 6.0),
            Point::new(3.0, 6.0),
            Point::new(3.0, 8.0),
            Point::new(1.0, 8.0),
            Point::new(1.0, 6.0),
        ];
        let holes: [&[Point]; 1] = [&hole];
        let c = Polygon::new(&SQUARE, &holes).centroid().unwrap();
        assert!(c.x > 5.0 && c.y < 5.0, "centroid {c:?}");
    }

    #[test]
    fn degenerate_ring_has_no_centroid() {
        let line = [Point::new(0.0, 0.0), Point::new(1.0, 1.0), Point::new(2.0, 2.0)];
        let poly = Polygon::from_exterior(&line);
        assert_eq!(poly.area(), 0.0);
        assert_eq!(poly.centroid(), None);
    }
}
