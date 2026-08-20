//! `tpt-gis-index` example: a region quadtree for fixed-bounds indexing, and
//! walking an S2 cell hierarchy (parent -> four children).
//!
//! Run with: `cargo run --example quadtree_demo -p tpt-gis-index`

use tpt_gis_geom::{Point, Rect};
use tpt_gis_index::quadtree::Quadtree;
use tpt_gis_index::s2::S2CellId;

fn main() {
    // A region quadtree needs its universe up front, unlike an R-Tree.
    let universe = Rect::new(Point::new(0.0, 0.0), Point::new(100.0, 100.0));
    let mut tree: Quadtree<&str> = Quadtree::new(universe);

    let items = [
        (Rect::new(Point::new(1.0, 1.0), Point::new(2.0, 2.0)), "nw"),
        (Rect::new(Point::new(60.0, 60.0), Point::new(61.0, 61.0)), "ne"),
        (Rect::new(Point::new(10.0, 1.0), Point::new(11.0, 2.0)), "sw"),
        (Rect::new(Point::new(90.0, 1.0), Point::new(91.0, 2.0)), "se"),
    ];
    for (bbox, label) in items {
        tree.insert(bbox, label);
    }
    println!("indexed {} items", tree.len());

    // Query the north-east region; only items fully inside it should match.
    let hits = tree.query(&Rect::new(Point::new(50.0, 50.0), Point::new(100.0, 100.0)));
    println!("NE query -> {hits:?}");

    // S2: descend a cell hierarchy from a coarse parent to its four children.
    let cell = S2CellId::from_lat_lon_degrees(40.0, -74.0).parent(8);
    println!("S2 parent (level {}) token: {}", cell.level(), cell.to_token());
    println!("four children at level {}:", cell.level() + 1);
    for child in cell.children() {
        let (lat, lon) = child.to_lat_lon_degrees();
        println!("  child {} -> center ({lat:.4}, {lon:.4})", child.to_token());
    }
}
