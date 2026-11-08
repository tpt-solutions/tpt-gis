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

    let rtree = build_polygon_index(polygons, config);

    let mut results = Vec::new();
    let mut candidates = Vec::new();

    // For each point, query the R-Tree for candidate polygons, then test exactly
    for point_data in points {
        for_each_containing(&rtree, polygons, point_data.point, &mut candidates, |poly_idx| {
            let poly = &polygons[poly_idx];
            results.push(JoinResult {
                point: point_data.point,
                point_payload: point_data.payload.clone(),
                polygon: poly.polygon.clone(),
                polygon_payload: poly.payload.clone(),
            });
        });
    }

    results
}

/// Index-only spatial join: like [`spatial_join`], but returns `(point index, polygon
/// index)` pairs into the input slices instead of cloning every matched polygon and
/// payload. Use this when polygons are large or matches are numerous; look the
/// geometry and payloads up by index afterwards.
///
/// Pairs are ordered by point index, then by R-Tree candidate order.
#[must_use]
pub fn spatial_join_indices<P, Q>(
    points: &[IndexedPoint<P>],
    polygons: &[IndexedPolygon<Q>],
    config: SpatialJoinConfig,
) -> Vec<(usize, usize)> {
    if points.is_empty() || polygons.is_empty() {
        return Vec::new();
    }

    let rtree = build_polygon_index(polygons, config);
    let mut results = Vec::new();
    let mut candidates = Vec::new();

    for (point_idx, point_data) in points.iter().enumerate() {
        for_each_containing(&rtree, polygons, point_data.point, &mut candidates, |poly_idx| {
            results.push((point_idx, poly_idx));
        });
    }

    results
}

/// Builds the R-Tree over polygon bounding boxes (values are indices into `polygons`).
fn build_polygon_index<Q>(
    polygons: &[IndexedPolygon<Q>],
    config: SpatialJoinConfig,
) -> RTree<usize> {
    if config.use_bulk_load {
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
    }
}

/// Calls `on_match` with the index of every polygon that contains `point`. `candidates`
/// is a caller-owned buffer reused across calls to avoid a per-point allocation.
fn for_each_containing<'t, Q>(
    rtree: &'t RTree<usize>,
    polygons: &[IndexedPolygon<Q>],
    point: Point,
    candidates: &mut Vec<&'t usize>,
    mut on_match: impl FnMut(usize),
) {
    candidates.clear();
    rtree.query_into(&Rect::new(point, point), candidates);
    for &&poly_idx in candidates.iter() {
        let mut scratch = Vec::new();
        if polygons[poly_idx].polygon.as_geom(&mut scratch).contains_point(point) {
            on_match(poly_idx);
        }
    }
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
    let rtree = build_polygon_index(polygons, config);

    // Process points one at a time (or in batches if the iterator provides them)
    let mut candidates = Vec::new();
    for point_data in points {
        for_each_containing(&rtree, polygons, point_data.point, &mut candidates, |poly_idx| {
            let poly = &polygons[poly_idx];
            on_match(JoinResult {
                point: point_data.point,
                point_payload: point_data.payload.clone(),
                polygon: poly.polygon.clone(),
                polygon_payload: poly.payload.clone(),
            });
        });
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

    let rtree = build_polygon_index(polygons, config);

    let mut batch = Vec::with_capacity(batch_size);

    let mut candidates = Vec::new();
    for point_data in points {
        for_each_containing(&rtree, polygons, point_data.point, &mut candidates, |poly_idx| {
            let poly = &polygons[poly_idx];
            batch.push(JoinResult {
                point: point_data.point,
                point_payload: point_data.payload.clone(),
                polygon: poly.polygon.clone(),
                polygon_payload: poly.payload.clone(),
            });

            if batch.len() >= batch_size {
                on_batch(core::mem::take(&mut batch));
            }
        });
    }

    if !batch.is_empty() {
        on_batch(batch);
    }
}

/// A pair of polygons whose areas (boundaries or interiors) meet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolygonPair {
    /// Index into the first polygon list.
    pub left: usize,
    /// Index into the second polygon list.
    pub right: usize,
}

/// Polygon-polygon intersection join. Returns every `(left, right)` index pair whose
/// polygons intersect, using the R-Tree on the right-hand bounding boxes to limit
/// candidates before the exact `polygons_intersect` test.
#[must_use]
pub fn polygon_intersection_join<P, Q>(
    left: &[IndexedPolygon<P>],
    right: &[IndexedPolygon<Q>],
) -> Vec<PolygonPair> {
    let entries: Vec<(Rect, usize)> =
        right.iter().enumerate().filter_map(|(j, p)| p.polygon.bbox().map(|b| (b, j))).collect();
    let tree = RTree::bulk_load(entries);

    let mut out = Vec::new();
    for (i, lp) in left.iter().enumerate() {
        let Some(bbox) = lp.polygon.bbox() else { continue };
        for &j in tree.query(&bbox) {
            let mut scratch_l = Vec::new();
            let mut scratch_r = Vec::new();
            let a = lp.polygon.as_geom(&mut scratch_l);
            let b = right[j].polygon.as_geom(&mut scratch_r);
            if tpt_gis_geom::predicates::polygons_intersect(&a, &b) {
                out.push(PolygonPair { left: i, right: j });
            }
        }
    }
    out
}

/// The nearest polygon to a point, with the exact distance to it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NearestMatch {
    /// Index of the point in the input slice.
    pub point: usize,
    /// Index of the nearest polygon in the input slice.
    pub polygon: usize,
    /// Planar distance from the point to the polygon (zero if it lies inside).
    pub distance: f64,
}

/// Exact planar distance from `point` to a polygon: zero if the point is inside the
/// polygon (holes excluded), otherwise the distance to the nearest ring edge.
#[must_use]
pub fn point_polygon_distance(point: Point, polygon: &IoPolygon) -> f64 {
    let mut scratch = Vec::new();
    let geom = polygon.as_geom(&mut scratch);
    if geom.contains_point(point) {
        return 0.0;
    }
    let mut best = f64::INFINITY;
    for ring in core::iter::once(geom.exterior).chain(geom.interiors.iter().copied()) {
        for i in 0..ring.len() {
            let a = ring[i];
            let b = ring[(i + 1) % ring.len()];
            best = best.min(tpt_gis_geom::distance::point_to_segment(point, a, b));
        }
    }
    best
}

/// For each point, the nearest polygon by exact distance.
///
/// Polygons are visited in order of their bounding-box distance, and the search for a
/// point stops as soon as no remaining box can be closer than the best exact match
/// found so far. Points with no polygons produce no result.
#[must_use]
pub fn nearest_polygon_join<P, Q>(
    points: &[IndexedPoint<P>],
    polygons: &[IndexedPolygon<Q>],
) -> Vec<NearestMatch> {
    let entries: Vec<(Rect, usize)> =
        polygons.iter().enumerate().filter_map(|(j, p)| p.polygon.bbox().map(|b| (b, j))).collect();
    if entries.is_empty() {
        return Vec::new();
    }
    let tree = RTree::bulk_load(entries);

    let mut out = Vec::new();
    for (i, pt) in points.iter().enumerate() {
        let mut best: Option<NearestMatch> = None;
        // Candidates in non-decreasing bounding-box distance; a box distance is a lower
        // bound on the exact distance, so the scan can stop once it exceeds `best`.
        for (box_dist, &j) in tree.nearest_iter(pt.point) {
            if best.is_some_and(|b| box_dist > b.distance) {
                break;
            }
            let d = point_polygon_distance(pt.point, &polygons[j].polygon);
            if best.is_none_or(|b| d < b.distance) {
                best = Some(NearestMatch { point: i, polygon: j, distance: d });
            }
        }
        out.extend(best);
    }
    out
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
    fn index_join_matches_full_join() {
        let square = |x: f64, y: f64, id| {
            make_polygon(
                vec![
                    Point::new(x, y),
                    Point::new(x + 10.0, y),
                    Point::new(x + 10.0, y + 10.0),
                    Point::new(x, y + 10.0),
                    Point::new(x, y),
                ],
                id,
            )
        };
        // Overlapping squares so some points match two polygons.
        let polygons = vec![square(0.0, 0.0, 0), square(5.0, 5.0, 1), square(30.0, 30.0, 2)];
        let points: Vec<_> = (0..40)
            .map(|i| make_point((i % 8) as f64 * 5.0 - 2.0, (i / 8) as f64 * 5.0 - 2.0, i))
            .collect();

        for bulk in [true, false] {
            let config = SpatialJoinConfig { use_bulk_load: bulk };
            let full = spatial_join(&points, &polygons, config);
            let pairs = spatial_join_indices(&points, &polygons, config);
            assert_eq!(full.len(), pairs.len());
            assert!(!pairs.is_empty());
            for (res, &(pi, qi)) in full.iter().zip(&pairs) {
                assert_eq!(res.point_payload, points[pi].payload);
                assert_eq!(res.polygon_payload, polygons[qi].payload);
            }
        }
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

#[cfg(test)]
mod polygon_join_tests {
    use super::*;
    use tpt_gis_io::geometry::Point as IoPoint;

    fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> IoPolygon {
        IoPolygon::from_exterior(vec![
            IoPoint::new(x0, y0),
            IoPoint::new(x1, y0),
            IoPoint::new(x1, y1),
            IoPoint::new(x0, y1),
            IoPoint::new(x0, y0),
        ])
    }

    fn indexed(polygon: IoPolygon, id: usize) -> IndexedPolygon<usize> {
        IndexedPolygon { polygon, payload: id }
    }

    #[test]
    fn polygon_join_finds_overlapping_pairs_only() {
        let left =
            vec![indexed(square(0.0, 0.0, 4.0, 4.0), 0), indexed(square(20.0, 0.0, 21.0, 1.0), 1)];
        let right = vec![
            indexed(square(3.0, 3.0, 6.0, 6.0), 0),
            indexed(square(10.0, 10.0, 11.0, 11.0), 1),
        ];
        let pairs = polygon_intersection_join(&left, &right);
        assert_eq!(pairs, vec![PolygonPair { left: 0, right: 0 }]);
    }

    #[test]
    fn point_inside_hole_is_at_hole_edge_distance() {
        // 10x10 square with a 2x2 hole at (4..6, 4..6). The point (5,5) is in the hole,
        // so it is 1.0 from the nearest hole edge, not zero.
        let exterior = vec![
            IoPoint::new(0.0, 0.0),
            IoPoint::new(10.0, 0.0),
            IoPoint::new(10.0, 10.0),
            IoPoint::new(0.0, 10.0),
            IoPoint::new(0.0, 0.0),
        ];
        let hole = vec![
            IoPoint::new(4.0, 4.0),
            IoPoint::new(6.0, 4.0),
            IoPoint::new(6.0, 6.0),
            IoPoint::new(4.0, 6.0),
            IoPoint::new(4.0, 4.0),
        ];
        let poly = IoPolygon { rings: vec![exterior, hole] };
        let d = point_polygon_distance(Point::new(5.0, 5.0), &poly);
        assert!((d - 1.0).abs() < 1e-9, "distance {d}");
        assert_eq!(point_polygon_distance(Point::new(1.0, 1.0), &poly), 0.0);
    }

    /// The nearest join must match a brute-force scan over all polygons.
    #[test]
    fn nearest_join_matches_brute_force() {
        let polygons: Vec<IndexedPolygon<usize>> = (0..40usize)
            .map(|i| {
                let x = ((i * 31) % 50) as f64;
                let y = ((i * 17) % 50) as f64;
                indexed(square(x, y, x + 2.0, y + 2.0), i)
            })
            .collect();
        let points: Vec<IndexedPoint<usize>> = (0..25usize)
            .map(|i| IndexedPoint {
                point: Point::new(((i * 13) % 60) as f64 + 0.3, ((i * 7) % 60) as f64 + 0.6),
                payload: i,
            })
            .collect();

        let matches = nearest_polygon_join(&points, &polygons);
        assert_eq!(matches.len(), points.len());
        for m in &matches {
            let p = points[m.point].point;
            let best = polygons
                .iter()
                .map(|q| point_polygon_distance(p, &q.polygon))
                .fold(f64::INFINITY, f64::min);
            assert!(
                (m.distance - best).abs() < 1e-9,
                "point {}: {} vs {}",
                m.point,
                m.distance,
                best
            );
        }
    }
}
