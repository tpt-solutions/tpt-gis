# CLI Usage

## spatial-join-cli

The `spatial-join-cli` example is a command-line tool for spatial operations.

### Subcommands

#### spatial-join

Run a spatial join between points and polygons.

```sh
cargo run -p spatial-join-cli -- spatial-join \
  --points <points.geojson> \
  --polygons <polygons.geojson> \
  [--output <output.geojson>] \
  [--format geojson|csv] \
  [--incremental]
```

- `--points`: GeoJSON `FeatureCollection` containing `Point` features.
- `--polygons`: GeoJSON `FeatureCollection` containing `Polygon` features.
- `--output`: Output file path. Writes to stdout if omitted.
- `--format`: Output format (`geojson` or `csv`). Default: `geojson`.
- `--incremental`: Use incremental R-Tree insert instead of bulk loading.

#### reproject

Reproject a GeoJSON `FeatureCollection` from WGS84 to a target CRS.

```sh
cargo run -p spatial-join-cli -- reproject \
  --input <input.geojson> \
  --crs <target-crs> \
  [--output <output.geojson>]
```

Supported target CRS values:
- `web-mercator` — WGS84 to Web Mercator (EPSG:3857)
- `utm-<zone><hemisphere>` — WGS84 to UTM, e.g. `utm-18n`, `utm-33s`

#### generate-fixtures

Generate synthetic GeoJSON test fixtures for benchmarking.

```sh
cargo run -p spatial-join-cli -- generate-fixtures \
  [--points <count>] \
  [--polygons <count>] \
  [--seed <value>] \
  --output <directory>
```

- `--points`: Number of random points to generate. Default: `10000`.
- `--polygons`: Number of random polygons to generate. Default: `500`.
- `--seed`: Random seed for reproducibility. Default: `42`.
- `--output`: Output directory for generated files.

### Benchmarking

Run the throughput benchmark:

```sh
cargo bench -p spatial-join-cli
```

This benchmarks spatial join with 10,000 points and 500 polygons using both bulk-load and streaming R-Tree strategies.
