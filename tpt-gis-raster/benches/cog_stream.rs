//! Benchmark for the Cloud Optimized GeoTIFF streaming reader.
//!
//! It compares two ways of reading a remote-style COG built entirely in memory
//! (served through [`MemoryTransport`], which still exercises the tile-by-tile
//! fetch path the HTTP reader uses):
//!
//! * `stream_region_one_tile` — fetch only the single tile overlapping a small
//!   window. This is what makes a COG "cloud optimized": a client that needs a
//!   viewport-sized slice downloads a fraction of the file.
//! * `full_decode` — fetch and decode the entire image.
//!
//! Run with `cargo bench -p tpt-gis-raster --features http --bench cog_stream`.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use tpt_gis_raster::geotiff::testsupport::make_cog;
use tpt_gis_raster::http::{CogRangeReader, MemoryTransport};

fn build_cog() -> Vec<u8> {
    // 1024x1024, 256x256 tiles -> 16 tiles, ~1 MiB of pixels. The 1 MiB prefix
    // covers the header plus the first ~16 tiles, so the single-tile region read
    // below stays a streaming (partial) fetch.
    make_cog(1024, 1024, 256, 256, |c, r| ((c as u32 * 7 + r as u32 * 13) % 251) as u8)
}

fn bench_stream(c: &mut Criterion) {
    let bytes = build_cog();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("tokio runtime");

    let mut group = c.benchmark_group("cog_read");
    group.bench_function(BenchmarkId::new("region", "one_tile"), |b| {
        b.iter(|| {
            let reader = CogRangeReader::with_transport(MemoryTransport::new(bytes.clone()));
            let band = runtime
                .block_on(reader.read_region(0, 0, 256, 256))
                .expect("region read");
            criterion::black_box(band.width());
        });
    });
    group.bench_function(BenchmarkId::new("full", "decode"), |b| {
        b.iter(|| {
            let reader = CogRangeReader::with_transport(MemoryTransport::new(bytes.clone()));
            let band = runtime.block_on(reader.read()).expect("full read");
            criterion::black_box(band.width());
        });
    });
    group.finish();
}

criterion_group!(benches, bench_stream);
criterion_main!(benches);
