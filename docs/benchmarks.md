# Spatial Join Throughput Benchmark

Results from `cargo bench -p spatial-join-cli` on Windows x86_64, debug build.

## Benchmark: 10,000 points × 500 polygons

| Strategy           | Time per join |
|--------------------|---------------|
| Bulk-load R-Tree   | ~4.35 ms      |
| Streaming R-Tree   | ~3.97 ms      |

## Methodology

- Points: 10,000 random coordinates in `[-1000, 1000] × [-1000, 1000]`
- Polygons: 500 random axis-aligned squares, size 10–100 units
- Random seed: 42 (reproducible)
- Both strategies query the same data; streaming accumulates results via callback

## Notes

- Bulk-load uses Sort-Tile-Recursive (STR) packing for better query performance
- Streaming processes points one-at-a-time through the same R-Tree
- Results include R-Tree candidate filtering + exact point-in-polygon tests
- Benchmark harness: criterion 0.5
