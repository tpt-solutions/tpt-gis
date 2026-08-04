//! Benchmarks a single geofence check (point-in-polygon + escape vector calculation)
//! on the host, as a stand-in for embedded latency characterization.
//!
//! This is *not* a substitute for measuring on real flight-controller hardware —
//! host CPU performance isn't representative of, say, a Cortex-M running at a few
//! hundred MHz — but it does establish that the check itself is cheap (no
//! allocation, a handful of trig calls), which is the property that matters most
//! for real-time use. True on-hardware latency measurement is tracked as a
//! follow-up (see `todo.md`).
#![allow(missing_docs)] // criterion's macros generate undocumented items.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use drone_geofence_demo::check_position;
use tpt_gis_core::GeoPoint;

fn bench_geofence_check(c: &mut Criterion) {
    let inside = GeoPoint::new(37.775, -122.415);
    let outside = GeoPoint::new(37.760, -122.430);

    c.bench_function("geofence_check_breach", |b| {
        b.iter(|| check_position(black_box(inside)));
    });

    c.bench_function("geofence_check_clear", |b| {
        b.iter(|| check_position(black_box(outside)));
    });
}

criterion_group!(benches, bench_geofence_check);
criterion_main!(benches);
