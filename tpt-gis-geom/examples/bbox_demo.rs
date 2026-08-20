//! `tpt-gis-geom` example: bounding boxes (envelopes), unions, intersection tests,
//! and nearest-distance / boundary queries.
//!
//! Run with: `cargo run --example bbox_demo -p tpt-gis-geom`

use tpt_gis_geom::distance;
use tpt_gis_geom::{LineString, Point, Polygon, Rect};

fn main() {
    let a = Rect::new(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
    let b = Rect::new(Point::new(5.0, 5.0), Point::new(15.0, 15.0));

    // Envelope (bounding box) of an arbitrary point set.
    let pts = [Point::new(-2.0, 3.0), Point::new(4.0, -1.0), Point::new(1.0, 5.0)];
    let env = Rect::from_points(&pts).unwrap();
    println!("envelope: ({}, {}) .. ({}, {})", env.min.x, env.min.y, env.max.x, env.max.y);

    // Union grows to cover both; intersection test reports overlap.
    let union = a.union(&b);
    println!(
        "a ∩ b overlap? {}; union spans ({}, {}) .. ({}, {})",
        a.intersects(&b),
        union.min.x,
        union.min.y,
        union.max.x,
        union.max.y
    );

    // Shortest distance from a point to a polyline.
    let path_coords = [Point::new(0.0, 0.0), Point::new(10.0, 0.0), Point::new(10.0, 10.0)];
    let path = LineString::new(&path_coords);
    let d = distance::point_to_line_string(Point::new(5.0, 5.0), &path);
    println!("distance from (5, 5) to the polyline: {d:.2}");

    // Closest boundary point of a polygon — e.g. the nearest way out of a zone.
    let square = [
        Point::new(0.0, 0.0),
        Point::new(10.0, 0.0),
        Point::new(10.0, 10.0),
        Point::new(0.0, 10.0),
        Point::new(0.0, 0.0),
    ];
    let poly = Polygon::from_exterior(&square);
    let nearest = poly.nearest_boundary_point(Point::new(2.0, 5.0)).unwrap();
    println!("nearest boundary point to (2, 5): ({}, {})", nearest.x, nearest.y);

    // Containment respects holes: a point in the hole is outside the polygon.
    let hole = [
        Point::new(4.0, 4.0),
        Point::new(6.0, 4.0),
        Point::new(6.0, 6.0),
        Point::new(4.0, 6.0),
        Point::new(4.0, 4.0),
    ];
    let interiors: [&[Point]; 1] = [&hole];
    let donut = Polygon::new(&square, &interiors);
    println!(
        "point (5, 5) in the hole is excluded? {}",
        !donut.contains_point(Point::new(5.0, 5.0))
    );
}
