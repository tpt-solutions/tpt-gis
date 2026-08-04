//! Property-based tests for geometry predicates and distances.
//!
//! These tests use `proptest` to verify invariants that must hold for all
//! valid inputs: symmetry of distance, commutativity of intersection,
//! triangle inequality, and idempotency of predicates.

#![allow(missing_docs)]

use proptest::prelude::*;
use tpt_gis_geom::{
    bbox::Rect, distance, line_string::LineString, point::Point, polygon::Polygon, predicates,
};

proptest! {
    #[test]
    fn point_distance_is_symmetric(
        ax in -1e6f64..1e6,
        ay in -1e6f64..1e6,
        bx in -1e6f64..1e6,
        by in -1e6f64..1e6,
    ) {
        let a = Point::new(ax, ay);
        let b = Point::new(bx, by);
        let dab = distance::point_to_point(a, b);
        let dba = distance::point_to_point(b, a);
        prop_assert!((dab - dba).abs() < 1e-6, "distance not symmetric: {} != {}", dab, dba);
    }

    #[test]
    fn point_distance_is_non_negative(
        ax in -1e6f64..1e6,
        ay in -1e6f64..1e6,
        bx in -1e6f64..1e6,
        by in -1e6f64..1e6,
    ) {
        let a = Point::new(ax, ay);
        let b = Point::new(bx, by);
        let d = distance::point_to_point(a, b);
        prop_assert!(d >= 0.0, "distance is negative: {}", d);
    }

    #[test]
    fn point_distance_satisfies_triangle_inequality(
        ax in -1e6f64..1e6,
        ay in -1e6f64..1e6,
        bx in -1e6f64..1e6,
        by in -1e6f64..1e6,
        cx in -1e6f64..1e6,
        cy in -1e6f64..1e6,
    ) {
        let a = Point::new(ax, ay);
        let b = Point::new(bx, by);
        let c = Point::new(cx, cy);
        let dab = distance::point_to_point(a, b);
        let dbc = distance::point_to_point(b, c);
        let dac = distance::point_to_point(a, c);
        prop_assert!(
            dac <= dab + dbc + 1e-6,
            "triangle inequality violated: {} > {} + {}", dac, dab, dbc
        );
    }

    #[test]
    fn point_distance_to_self_is_zero(
        x in -1e6f64..1e6,
        y in -1e6f64..1e6,
    ) {
        let p = Point::new(x, y);
        let d = distance::point_to_point(p, p);
        prop_assert!(d < 1e-9, "distance to self is not zero: {}", d);
    }

    #[test]
    fn closest_point_on_segment_is_within_segment_bounds(
        px in -1e6f64..1e6,
        py in -1e6f64..1e6,
        ax in -1e6f64..1e6,
        ay in -1e6f64..1e6,
        bx in -1e6f64..1e6,
        by in -1e6f64..1e6,
    ) {
        let point = Point::new(px, py);
        let a = Point::new(ax, ay);
        let b = Point::new(bx, by);
        let closest = distance::closest_point_on_segment(point, a, b);
        let min_x = a.x.min(b.x);
        let max_x = a.x.max(b.x);
        let min_y = a.y.min(b.y);
        let max_y = a.y.max(b.y);
        prop_assert!(
            closest.x >= min_x - 1e-6 && closest.x <= max_x + 1e-6 &&
            closest.y >= min_y - 1e-6 && closest.y <= max_y + 1e-6,
            "closest point ({}, {}) outside segment bbox [{}, {}] x [{}, {}]",
            closest.x, closest.y, min_x, max_x, min_y, max_y
        );
    }

    #[test]
    fn bbox_contains_all_input_points(pts in prop::collection::vec((-1e6f64..1e6, -1e6f64..1e6), 1..20)) {
        let pts: Vec<Point> = pts.into_iter().map(|(x, y)| Point::new(x, y)).collect();
        let bbox = Rect::from_points(&pts).unwrap();
        for p in &pts {
            prop_assert!(
                bbox.contains_point(*p),
                "bbox does not contain point ({}, {})", p.x, p.y
            );
        }
    }

    #[test]
    fn bbox_union_is_commutative(
        ax in -1e6f64..1e6,
        ay in -1e6f64..1e6,
        bx in -1e6f64..1e6,
        by in -1e6f64..1e6,
        cx in -1e6f64..1e6,
        cy in -1e6f64..1e6,
        dx in -1e6f64..1e6,
        dy in -1e6f64..1e6,
    ) {
        let rect_a = Rect::new(Point::new(ax, ay), Point::new(bx, by));
        let rect_b = Rect::new(Point::new(cx, cy), Point::new(dx, dy));
        prop_assert_eq!(rect_a.union(&rect_b), rect_b.union(&rect_a));
    }

    #[test]
    fn bbox_intersects_is_commutative(
        ax in -1e6f64..1e6,
        ay in -1e6f64..1e6,
        bx in -1e6f64..1e6,
        by in -1e6f64..1e6,
        cx in -1e6f64..1e6,
        cy in -1e6f64..1e6,
        dx in -1e6f64..1e6,
        dy in -1e6f64..1e6,
    ) {
        let rect_a = Rect::new(Point::new(ax, ay), Point::new(bx, by));
        let rect_b = Rect::new(Point::new(cx, cy), Point::new(dx, dy));
        prop_assert_eq!(
            rect_a.intersects(&rect_b),
            rect_b.intersects(&rect_a)
        );
    }

    #[test]
    fn segment_intersection_is_commutative(
        p1x in -1e6f64..1e6,
        p1y in -1e6f64..1e6,
        q1x in -1e6f64..1e6,
        q1y in -1e6f64..1e6,
        p2x in -1e6f64..1e6,
        p2y in -1e6f64..1e6,
        q2x in -1e6f64..1e6,
        q2y in -1e6f64..1e6,
    ) {
        let p1 = Point::new(p1x, p1y);
        let q1 = Point::new(q1x, q1y);
        let p2 = Point::new(p2x, p2y);
        let q2 = Point::new(q2x, q2y);
        prop_assert_eq!(
            predicates::segments_intersect(p1, q1, p2, q2),
            predicates::segments_intersect(p2, q2, p1, q1)
        );
    }

    #[test]
    fn linestring_length_is_non_negative(pts in prop::collection::vec((-1e6f64..1e6, -1e6f64..1e6), 0..20)) {
        let pts: Vec<Point> = pts.into_iter().map(|(x, y)| Point::new(x, y)).collect();
        let line = LineString::new(&pts);
        prop_assert!(line.length() >= 0.0);
    }

    #[test]
    fn linestring_with_repeated_points_has_zero_length(
        x in -1e6f64..1e6,
        y in -1e6f64..1e6,
        n in 1..10usize,
    ) {
        let first = Point::new(x, y);
        let repeated = vec![first; n];
        let line = LineString::new(&repeated);
        prop_assert!(line.length() < 1e-9);
    }

    #[test]
    fn point_in_polygon_is_idempotent(
        poly_pts in convex_polygon(),
        px in -1e6f64..1e6,
        py in -1e6f64..1e6,
    ) {
        let point = Point::new(px, py);
        let polygon = Polygon::from_exterior(&poly_pts);
        let c1 = polygon.contains_point(point);
        let c2 = polygon.contains_point(point);
        prop_assert_eq!(c1, c2, "contains_point is not idempotent");
    }

    #[test]
    fn polygon_bbox_contains_exterior_points(poly_pts in convex_polygon()) {
        let exterior = &poly_pts[..poly_pts.len() - 1];
        let polygon = Polygon::from_exterior(exterior);
        let bbox = polygon.bbox().unwrap();
        for p in exterior {
            prop_assert!(
                bbox.contains_point(*p),
                "polygon bbox does not contain exterior point ({}, {})", p.x, p.y
            );
        }
    }
}

/// Strategy that generates a simple (usually convex) polygon as a closed ring
/// (first point repeated at the end).
fn convex_polygon() -> impl Strategy<Value = Vec<Point>> {
    (0.0..1000.0, 0.0..1000.0, 10.0..500.0, 3..20usize, 0.1..0.5).prop_map(
        |(cx, cy, base_r, n, perturb)| {
            let mut pts = Vec::with_capacity(n + 1);
            for i in 0..n {
                let angle = (i as f64) / (n as f64) * std::f64::consts::TAU;
                let r = base_r * (1.0 + (i as f64 * 0.37).sin() * perturb);
                pts.push(Point::new(cx + r * angle.cos(), cy + r * angle.sin()));
            }
            pts.push(pts[0]);
            pts
        },
    )
}
