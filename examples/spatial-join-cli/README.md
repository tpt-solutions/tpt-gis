# spatial-join-cli

**MVP 1 of `tpt-gis`** — a planetary-scale points-in-polygons spatial join engine CLI,
built on the `tpt-gis-index` R-Tree and the `tpt-gis-core` / `tpt-gis-geom` /
`tpt-gis-io` crates.

This is a demo binary (`publish = false`) — it is not published to crates.io.

## Subcommands

### `spatial-join`

Reads points and polygons from GeoJSON `FeatureCollection` files, runs an
R-Tree-accelerated points-in-polygons join, and writes the matches as GeoJSON or CSV.

```sh
cargo run -p spatial-join-cli -- spatial-join \
    --points points.geojson \
    --polygons polygons.geojson \
    --output joined.geojson \
    --format geojson
```

- `--incremental` uses incremental R-Tree insertion instead of bulk loading.
- Omit `--output` to print to stdout.

### `reproject`

Reprojects a GeoJSON `FeatureCollection` from WGS84 to a target CRS
(`web-mercator` or a UTM zone like `utm-18n` / `utm-33s`).

```sh
cargo run -p spatial-join-cli -- reproject \
    --input input.geojson \
    --crs web-mercator \
    --output reprojected.geojson
```

### `generate-fixtures`

Writes synthetic GeoJSON test fixtures for benchmarking and development.

```sh
cargo run -p spatial-join-cli -- generate-fixtures \
    --points 10000 --polygons 500 --seed 42 --output fixtures/
```

## Throughput benchmark

A criterion benchmark (`spatial_join`) measures join throughput:

```sh
cargo bench -p spatial-join-cli
```

See `todo.md` (Phase 2 / MVP 1) for the baseline numbers.
