//! R-Tree-accelerated point-in-polygon spatial join.
//!
//! This module provides a high-performance spatial join between a set of points
//! and a set of polygons, using an R-Tree index for fast candidate filtering
//! followed by exact point-in-polygon tests from `tpt-gis-geom`.

use crate::rtree::RTree;
use tpt_gis_geom::{Point, Rect};
use tpt_gis_io::geometry::Polygon as IoPolygon;

/// A point with an associated payload value.
#[derive(Debug, Clone)]
pub struct IndexedPoint<T> {
    /// The point's coordinates.
    pub point: Point,
    /// User-defined payload (e.g., an ID, feature attributes, etc.).
    pub payload: T,
}

/// A polygon with an associated payload value.
#[derive(Debug, Clone)]
pub struct IndexedPolygon<T> {
    /// The polygon geometry (owned, for use with tpt-gis-io parsed data).
    pub polygon: IoPolygon,
    /// User-defined payload.
    pub payload: T,
}

/// Result of a spatial join: a point-polygon pair that satisfies the predicate.
#[derive(Debug, Clone)]
pub struct JoinResult<P, Q> {
    /// The point that was found inside the polygon.
    pub point: Point,
    /// The point's payload.
    pub point_payload: P,
    /// The polygon that contains the point.
    pub polygon: IoPolygon,
    /// The polygon's payload.
    pub polygon_payload: Q,
}

/// Configuration for the spatial join.
#[derive(Debug, Clone, Copy)]
pub struct SpatialJoinConfig {
    /// Whether to use bulk loading for the R-Tree (faster for large datasets).
    pub use_bulk_load: bool,
}

impl Default for SpatialJoinConfig {
    fn default() -> Self {
        Self { use_bulk_load: true }
    }
}

/// Performs a spatial join: finds all (point, polygon) pairs where the point
/// lies inside the polygon.
///
/// This uses an R-Tree index on the polygons' bounding boxes for fast
/// candidate filtering, then applies exact point-in-polygon tests.
///
/// # Arguments
/// * `points` - Points to test, each with a payload.
/// * `polygons` - Polygons to test against, each with a payload.
/// * `config` - Optional configuration (bulk load vs incremental insert).
///
/// # Returns
/// A vector of `JoinResult` containing all matching point-polygon pairs.
pub fn spatial_join<P, Q>(
    points: &[IndexedPoint<P>],
    polygons: &[IndexedPolygon<Q>],
    config: SpatialJoinConfig,
) -> Vec<JoinResult<P, Q>>
where
    P: Clone,
    Q: Clone,
{
    if points.is_empty() || polygons.is_empty() {
        return Vec::new();
    }

    // Build R-Tree from polygon bounding boxes
    let rtree = if config.use_bulk_load {
        let entries: Vec<(Rect, usize)> = polygons
            .iter()
            .enumerate()
            .filter_map(|(idx, poly)| poly.polygon.bbox().map(|bbox| (bbox, idx)))
            .collect();
        RTree::bulk_load(entries)
    } else {
        let mut tree = RTree::new();
        for (idx, poly) in polygons.iter().enumerate() {
            if let Some(bbox) = poly.polygon.bbox() {
                tree.insert(bbox, idx);
            }
        }
        tree
    };

    let mut results = Vec::new();

    // For each point, query the R-Tree for candidate polygons, then test exactly
    for point_data in points {
        let point_rect = Rect::new(point_data.point, point_data.point);
        let candidate_indices = rtree.query(&point_rect);

        for &poly_idx in candidate_indices {
            let poly = &polygons[poly_idx];
            // Convert to tpt-gis-geom Polygon for exact test
            let mut scratch = Vec::new();
            let geom_poly = poly.polygon.as_geom(&mut scratch);
            if geom_poly.contains_point(point_data.point) {
                results.push(JoinResult {
                    point: point_data.point,
                    point_payload: point_data.payload.clone(),
                    polygon: poly.polygon.clone(),
                    polygon_payload: poly.payload.clone(),
                });
            }
        }
    }

    results
}

/// Streaming spatial join for large datasets that don't fit in memory.
///
/// This processes points in batches, keeping only the polygon R-Tree in memory.
/// Useful when the point dataset is very large (e.g., 10M+ points) but the
/// polygon dataset is moderate (e.g., 50k polygons).
pub fn spatial_join_streaming<P, Q, F>(
    points: impl Iterator<Item = IndexedPoint<P>>,
    polygons: &[IndexedPolygon<Q>],
    config: SpatialJoinConfig,
    mut on_match: F,
) where
    P: Clone,
    Q: Clone,
    F: FnMut(JoinResult<P, Q>),
{
    if polygons.is_empty() {
        return;
    }

    // Build R-Tree once from polygons
    let rtree = if config.use_bulk_load {
        let entries: Vec<(Rect, usize)> = polygons
            .iter()
            .enumerate()
            .filter_map(|(idx, poly)| poly.polygon.bbox().map(|bbox| (bbox, idx)))
            .collect();
        RTree::bulk_load(entries)
    } else {
        let mut tree = RTree::new();
        for (idx, poly) in polygons.iter().enumerate() {
            if let Some(bbox) = poly.polygon.bbox() {
                tree.insert(bbox, idx);
            }
        }
        tree
    };

    // Process points one at a time (or in batches if the iterator provides them)
    for point_data in points {
        let point_rect = Rect::new(point_data.point, point_data.point);
        let candidate_indices = rtree.query(&point_rect);

        for &poly_idx in candidate_indices {
            let poly = &polygons[poly_idx];
            let mut scratch = Vec::new();
            let geom_poly = poly.polygon.as_geom(&mut scratch);
            if geom_poly.contains_point(point_data.point) {
                on_match(JoinResult {
                    point: point_data.point,
                    point_payload: point_data.payload.clone(),
                    polygon: poly.polygon.clone(),
                    polygon_payload: poly.payload.clone(),
                });
            }
        }
    }
}

/// Batch spatial join: processes points in chunks, calling a callback for each batch of results.
///
/// This is a middle ground between the fully in-memory `spatial_join` and the
/// fully streaming `spatial_join_streaming`. It accumulates results in batches
/// of `batch_size` before calling the callback, reducing callback overhead.
pub fn spatial_join_batched<P, Q, F>(
    points: &[IndexedPoint<P>],
    polygons: &[IndexedPolygon<Q>],
    config: SpatialJoinConfig,
    batch_size: usize,
    mut on_batch: F,
) where
    P: Clone,
    Q: Clone,
    F: FnMut(Vec<JoinResult<P, Q>>),
{
    if points.is_empty() || polygons.is_empty() || batch_size == 0 {
        return;
    }

    let rtree = if config.use_bulk_load {
        let entries: Vec<(Rect, usize)> = polygons
            .iter()
            .enumerate()
            .filter_map(|(idx, poly)| poly.polygon.bbox().map(|bbox| (bbox, idx)))
            .collect();
        RTree::bulk_load(entries)
    } else {
        let mut tree = RTree::new();
        for (idx, poly) in polygons.iter().enumerate() {
            if let Some(bbox) = poly.polygon.bbox() {
                tree.insert(bbox, idx);
            }
        }
        tree
    };

    let mut batch = Vec::with_capacity(batch_size);

    for point_data in points {
        let point_rect = Rect::new(point_data.point, point_data.point);
        let candidate_indices = rtree.query(&point_rect);

        for &poly_idx in candidate_indices {
            let poly = &polygons[poly_idx];
            let mut scratch = Vec::new();
            let geom_poly = poly.polygon.as_geom(&mut scratch);
            if geom_poly.contains_point(point_data.point) {
                batch.push(JoinResult {
                    point: point_data.point,
                    point_payload: point_data.payload.clone(),
                    polygon: poly.polygon.clone(),
                    polygon_payload: poly.payload.clone(),
                });

                if batch.len() >= batch_size {
                    on_batch(core::mem::take(&mut batch));
                }
            }
        }
    }

    if !batch.is_empty() {
        on_batch(batch);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_gis_io::geometry::{Point, Polygon};

    fn make_point(x: f64, y: f64, id: usize) -> IndexedPoint<usize> {
        IndexedPoint { point: Point::new(x, y), payload: id }
    }

    fn make_polygon(exterior: Vec<Point>, id: usize) -> IndexedPolygon<usize> {
        IndexedPolygon { polygon: Polygon::from_exterior(exterior), payload: id }
    }

    #[test]
    fn basic_spatial_join() {
        let points = vec![
            make_point(1.0, 1.0, 0),     // inside poly 0
            make_point(5.0, 5.0, 1),     // inside poly 0
            make_point(15.0, 15.0, 2),   // inside poly 1
            make_point(100.0, 100.0, 3), // outside all
        ];

        let polygons = vec![
            make_polygon(
                vec![
                    Point::new(0.0, 0.0),
                    Point::new(10.0, 0.0),
                    Point::new(10.0, 10.0),
                    Point::new(0.0, 10.0),
                    Point::new(0.0, 0.0),
                ],
                0,
            ),
            make_polygon(
                vec![
                    Point::new(10.0, 10.0),
                    Point::new(20.0, 10.0),
                    Point::new(20.0, 20.0),
                    Point::new(10.0, 20.0),
                    Point::new(10.0, 10.0),
                ],
                1,
            ),
        ];

        let results = spatial_join(&points, &polygons, SpatialJoinConfig::default());

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].point_payload, 0);
        assert_eq!(results[0].polygon_payload, 0);
        assert_eq!(results[1].point_payload, 1);
        assert_eq!(results[1].polygon_payload, 0);
        assert_eq!(results[2].point_payload, 2);
        assert_eq!(results[2].polygon_payload, 1);
    }

    #[test]
    fn spatial_join_with_holes() {
        let points = vec![
            make_point(1.0, 1.0, 0), // inside exterior
            make_point(5.0, 5.0, 1), // inside hole
        ];

        let exterior = vec![
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 10.0),
            Point::new(0.0, 10.0),
            Point::new(0.0, 0.0),
        ];
        let hole = vec![
            Point::new(4.0, 4.0),
            Point::new(6.0, 4.0),
            Point::new(6.0, 6.0),
            Point::new(4.0, 6.0),
            Point::new(4.0, 4.0),
        ];

        let mut poly = Polygon::from_exterior(exterior);
        poly.rings.push(hole);

        let polygons = vec![IndexedPolygon { polygon: poly, payload: 0 }];

        let results = spatial_join(&points, &polygons, SpatialJoinConfig::default());

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].point_payload, 0); // only the point outside the hole
    }

    #[test]
    fn test_spatial_join_streaming() {
        let points =
            vec![make_point(1.0, 1.0, 0), make_point(5.0, 5.0, 1), make_point(15.0, 15.0, 2)];

        let polygons = vec![
            make_polygon(
                vec![
                    Point::new(0.0, 0.0),
                    Point::new(10.0, 0.0),
                    Point::new(10.0, 10.0),
                    Point::new(0.0, 10.0),
                    Point::new(0.0, 0.0),
                ],
                0,
            ),
            make_polygon(
                vec![
                    Point::new(10.0, 10.0),
                    Point::new(20.0, 10.0),
                    Point::new(20.0, 20.0),
                    Point::new(10.0, 20.0),
                    Point::new(10.0, 10.0),
                ],
                1,
            ),
        ];

        let mut results = Vec::new();
        spatial_join_streaming(points.into_iter(), &polygons, SpatialJoinConfig::default(), |r| {
            results.push(r)
        });

        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_spatial_join_batched() {
        let points = vec![
            make_point(1.0, 1.0, 0),
            make_point(5.0, 5.0, 1),
            make_point(15.0, 15.0, 2),
            make_point(2.0, 2.0, 3),
        ];

        let polygons = vec![make_polygon(
            vec![
                Point::new(0.0, 0.0),
                Point::new(10.0, 0.0),
                Point::new(10.0, 10.0),
                Point::new(0.0, 10.0),
                Point::new(0.0, 0.0),
            ],
            0,
        )];

        let mut all_results = Vec::new();
        spatial_join_batched(
            &points,
            &polygons,
            SpatialJoinConfig::default(),
            2, // batch size
            |batch| all_results.extend(batch),
        );

        assert_eq!(all_results.len(), 3); // 3 points inside the polygon
    }

    #[test]
    fn empty_inputs() {
        let points: Vec<IndexedPoint<usize>> = vec![];
        let polygons: Vec<IndexedPolygon<usize>> = vec![];

        let results = spatial_join(&points, &polygons, SpatialJoinConfig::default());
        assert!(results.is_empty());

        let points = vec![make_point(1.0, 1.0, 0)];
        let results = spatial_join(&points, &polygons, SpatialJoinConfig::default());
        assert!(results.is_empty());

        let polygons = vec![make_polygon(
            vec![
                Point::new(0.0, 0.0),
                Point::new(10.0, 0.0),
                Point::new(10.0, 10.0),
                Point::new(0.0, 10.0),
                Point::new(0.0, 0.0),
            ],
            0,
        )];
        let results = spatial_join(&points, &polygons, SpatialJoinConfig::default());
        assert_eq!(results.len(), 1);
    }
}
