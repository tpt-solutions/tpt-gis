# Benchmarks

All numbers below are from `cargo bench` (criterion, which builds with the optimised bench
profile), on Windows x86_64. They are not a comparison with other GIS tools. Each run
prints a confidence interval; the middle value is quoted here.

## Spatial join: 10,000 points × 500 polygons

Command: `cargo bench -p spatial-join-cli --bench spatial_join`

| Strategy           | Time per join |
|--------------------|---------------|
| Bulk-load R-Tree   | ~3.75 ms      |
| Streaming R-Tree   | ~2.85 ms      |

- Points: 10,000 random coordinates in `[-1000, 1000] × [-1000, 1000]`
- Polygons: 500 random axis-aligned squares, size 10–100 units
- Random seed: 42 (reproducible)
- Bulk-load uses Sort-Tile-Recursive (STR) packing. Streaming inserts one point at a time
  through the same R-Tree, and accumulates results through a callback.
- Results include R-Tree candidate filtering and the exact point-in-polygon test.

## Drone geofence check

Command: `cargo bench -p drone-geofence-demo --bench geofence_check`

| Case                              | Time per check |
|-----------------------------------|----------------|
| Position outside the zone (clear) | ~96 ns         |
| Position inside the zone (breach) | ~657 ns        |

This is a host-side stand-in for the on-device check. The on-ESP32 timing is not yet measured
(see [`todo.md`](../todo.md), MVP 2).

## COG region read

Command: `cargo bench -p tpt-gis-raster --features http --bench cog_stream`

| Case                                          | Time        |
|-----------------------------------------------|-------------|
| Read one tile-sized region (`cog_read/region/one_tile`) | ~0.68 ms |
| Decode the full 1024 × 1024 image (`cog_read/full/decode`) | ~2.11 ms |

A region read is about 3× cheaper than a full decode, because it touches only the tiles the
window needs.

## Not yet done

- A release-mode comparison with GDAL, PostGIS or geopandas on 10 million points. The
  numbers above are for 10,000 points only.
- A benchmark of the HTTP range reader against a real server.
