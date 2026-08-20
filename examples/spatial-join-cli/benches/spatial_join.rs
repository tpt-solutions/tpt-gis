#![allow(missing_docs)]

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tpt_gis_cli::rng::SmallRng;
use tpt_gis_geom::Point;
use tpt_gis_index::spatial_join::{spatial_join, IndexedPoint, IndexedPolygon, SpatialJoinConfig};
use tpt_gis_io::geometry::Polygon as IoPolygon;

fn generate_points(n: usize, seed: u64) -> Vec<IndexedPoint<i32>> {
    let mut rng = SmallRng::seed_from_u64(seed);
    (0..n)
        .map(|i| IndexedPoint {
            point: Point::new(rng.gen_range(-1000.0, 1000.0), rng.gen_range(-1000.0, 1000.0)),
            payload: i as i32,
        })
        .collect()
}

fn generate_polygons(n: usize, seed: u64) -> Vec<IndexedPolygon<i32>> {
    let mut rng = SmallRng::seed_from_u64(seed);
    (0..n)
        .map(|i| {
            let cx = rng.gen_range(-900.0, 900.0);
            let cy = rng.gen_range(-900.0, 900.0);
            let size = rng.gen_range(10.0, 100.0);
            let exterior = vec![
                Point::new(cx - size, cy - size),
                Point::new(cx + size, cy - size),
                Point::new(cx + size, cy + size),
                Point::new(cx - size, cy + size),
                Point::new(cx - size, cy - size),
            ];
            IndexedPolygon { polygon: IoPolygon::from_exterior(exterior), payload: i as i32 }
        })
        .collect()
}

fn bench_spatial_join(c: &mut Criterion) {
    let points = generate_points(10_000, 42);
    let polygons = generate_polygons(500, 99);
    let config = SpatialJoinConfig { use_bulk_load: true };

    c.bench_function("spatial_join_10k_points_500_polygons", |b| {
        b.iter(|| {
            let results = spatial_join(black_box(&points), black_box(&polygons), config);
            black_box(results.len())
        })
    });
}

fn bench_spatial_join_streaming(c: &mut Criterion) {
    let points = generate_points(10_000, 42);
    let polygons = generate_polygons(500, 99);
    let config = SpatialJoinConfig { use_bulk_load: true };

    c.bench_function("spatial_join_streaming_10k_points_500_polygons", |b| {
        b.iter(|| {
            let mut count = 0usize;
            tpt_gis_index::spatial_join::spatial_join_streaming(
                black_box(points.iter().cloned()),
                black_box(&polygons),
                config,
                |_| count += 1,
            );
            black_box(count)
        })
    });
}

criterion_group!(benches, bench_spatial_join, bench_spatial_join_streaming);
criterion_main!(benches);
