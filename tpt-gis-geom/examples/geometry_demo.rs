//! `tpt-gis-geom` example: geometry primitives and topological predicates.
//!
//! Run with: `cargo run --example geometry_demo -p tpt-gis-geom`

use tpt_gis_geom::distance;
use tpt_gis_geom::predicates;
use tpt_gis_geom::{LineString, Point, Polygon, Rect};

fn main() {
    let p = Point::new(1.0, 2.0);

    // Bounding-box membership.
    let bbox = Rect::new(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
    println!("point inside bbox: {}", bbox.contains_point(p));

    // Line length.
    let line_pts = [Point::new(0.0, 0.0), Point::new(3.0, 4.0), Point::new(3.0, 0.0)];
    let line = LineString::new(&line_pts);
    println!("line length: {:.1}", line.length());

    // Point-to-segment distance.
    let d = distance::point_to_segment(p, Point::new(0.0, 0.0), Point::new(10.0, 0.0));
    println!("distance point->segment: {:.2}", d);

    // Polygon containment.
    let square = [
        Point::new(0.0, 0.0),
        Point::new(10.0, 0.0),
        Point::new(10.0, 10.0),
        Point::new(0.0, 10.0),
        Point::new(0.0, 0.0),
    ];
    let poly = Polygon::from_exterior(&square);
    println!("origin inside square: {}", poly.contains_point(Point::new(5.0, 5.0)));
    println!("outside point inside square: {}", poly.contains_point(Point::new(15.0, 5.0)));

    // Topological predicate: do two squares intersect?
    let other = [
        Point::new(5.0, 5.0),
        Point::new(15.0, 5.0),
        Point::new(15.0, 15.0),
        Point::new(5.0, 15.0),
        Point::new(5.0, 5.0),
    ];
    let poly2 = Polygon::from_exterior(&other);
    println!("squares intersect: {}", predicates::polygons_intersect(&poly, &poly2));

    // Nearest boundary point inside the polygon (e.g. an escape point out of a zone).
    let nearest = poly.nearest_boundary_point(Point::new(2.0, 5.0)).unwrap();
    println!("nearest boundary point to (2,5): ({}, {})", nearest.x, nearest.y);
}
