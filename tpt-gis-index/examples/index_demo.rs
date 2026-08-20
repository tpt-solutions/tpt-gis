//! `tpt-gis-index` example: R-Tree, H3, S2, and a spatial join.
//!
//! Run with: `cargo run --example index_demo -p tpt-gis-index`

use tpt_gis_geom::{Point, Rect};
use tpt_gis_index::h3;
use tpt_gis_index::s2::S2CellId;
use tpt_gis_index::spatial_join::{spatial_join, IndexedPoint, IndexedPolygon, SpatialJoinConfig};
use tpt_gis_index::RTree;
use tpt_gis_io::geometry::{Point as IoPoint, Polygon as IoPolygon};

fn main() {
    // --- R-Tree: bulk load + query -------------------------------------------
    let entries = vec![
        (Rect::new(Point::new(0.0, 0.0), Point::new(1.0, 1.0)), "a"),
        (Rect::new(Point::new(5.0, 5.0), Point::new(6.0, 6.0)), "b"),
        (Rect::new(Point::new(10.0, 10.0), Point::new(11.0, 11.0)), "c"),
    ];
    let tree = RTree::bulk_load(entries);
    let hits = tree.query(&Rect::new(Point::new(-1.0, -1.0), Point::new(2.0, 2.0)));
    println!("R-Tree query hits: {:?}", hits);

    // --- H3 hexagonal grid ---------------------------------------------------
    let cell = h3::cell_at(37.7749, -122.4194, 9).unwrap();
    println!("H3 cell: {} (resolution {})", cell, u8::from(cell.resolution()));
    let center = h3::cell_center(cell);
    println!("H3 cell center: ({}, {})", center.x, center.y);
    println!("H3 k=2 ring size: {}", h3::k_ring(cell, 2).len());

    // --- S2 cell tokens ------------------------------------------------------
    let id = S2CellId::from_lat_lon_degrees(40.743, -74.0008);
    println!("S2 token: {}", id.to_token());
    let coarse = id.parent(10);
    println!("S2 parent token: {}", coarse.to_token());
    assert_eq!(S2CellId::from_token(&coarse.to_token()), Some(coarse));

    // --- Spatial join: points in polygons ------------------------------------
    let points = vec![
        IndexedPoint { point: Point::new(1.0, 1.0), payload: 0usize },
        IndexedPoint { point: Point::new(15.0, 15.0), payload: 1usize },
    ];
    let polygons = vec![IndexedPolygon {
        polygon: IoPolygon::from_exterior(vec![
            IoPoint::new(0.0, 0.0),
            IoPoint::new(10.0, 0.0),
            IoPoint::new(10.0, 10.0),
            IoPoint::new(0.0, 10.0),
            IoPoint::new(0.0, 0.0),
        ]),
        payload: 100usize,
    }];
    let results = spatial_join(&points, &polygons, SpatialJoinConfig::default());
    let (pp, gp) =
        results.first().map_or((0usize, 0usize), |r| (r.point_payload, r.polygon_payload));
    println!("spatial join matches: {} (point {}, polygon {})", results.len(), pp, gp);
}
