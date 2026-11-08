# spatial-join-cli

**MVP 1 of `tpt-gis`** — a planetary-scale points-in-polygons spatial join engine, built on the
`tpt-gis-index` R-tree and the `tpt-gis-core` / `tpt-gis-geom` / `tpt-gis-io` crates.

This example is a thin wrapper around the same subcommands as the published CLI
[`tpt-gis-cli`](../../tpt-gis-cli/README.md) (binary `tptgis`). It also hosts the spatial-join
criterion benchmark, which is kept here so the published crate stays lean. For the full
reference, see the `tpt-gis-cli` README. Run `cargo run -p spatial-join-cli -- --help` to list
the subcommands.

## Subcommands

### `spatial-join`

Reads points and polygons from GeoJSON `FeatureCollection` files, runs an R-tree-accelerated
points-in-polygons join, and writes the matches as GeoJSON or CSV.

```sh
cargo run -p spatial-join-cli -- spatial-join \
    --points points.geojson \
    --polygons polygons.geojson \
    --output joined.geojson \
    --format geojson
```

- `--format csv` writes CSV instead of GeoJSON.
- `--incremental` uses incremental R-tree insertion instead of bulk loading.
- Omit `--output` to print to stdout.

### `reproject`

Reprojects a GeoJSON `FeatureCollection` from WGS84 to a target CRS (`web-mercator`, a UTM zone
such as `utm-18n` or `utm-33s`, or an EPSG code).

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
