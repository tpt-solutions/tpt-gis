# tpt-gis-cli

Command-line tool for the [`tpt-gis`](https://github.com/tpt-solutions/tpt-gis) planetary-scale GIS
engine. The binary is called `tptgis`.

It runs spatial joins, reprojects and converts vector files, summarises and validates
datasets, and compiles polygons into `no_std` Rust for embedded geofences — all without
GDAL, PROJ or GEOS.

> **Status:** not yet published to crates.io. Install from a checkout.

## Installation

```sh
cargo install --path tpt-gis-cli
tptgis --version
```

Or run it in place from the workspace root:

```sh
cargo run -p tpt-gis-cli --bin tptgis -- --help
```

Shell completions are printed by `tptgis completions <shell>` for `bash`, `zsh`, `fish`,
`powershell` and `elvish`:

```sh
tptgis completions bash > ~/.local/share/bash-completion/completions/tptgis
```

## Commands

| Command | Purpose |
| ------- | ------- |
| `spatial-join` | Points-in-polygons join from GeoJSON, as GeoJSON or CSV |
| `reproject` | Reproject a GeoJSON file from WGS84 |
| `convert` | Convert between GeoJSON, WKT, WKB, Shapefile and GeoPackage |
| `info` | Print a JSON summary of a vector file |
| `validate` | Check a vector file or GeoTIFF; non-zero exit on any problem |
| `geofence-compile` | Emit a `no_std` Rust module of polygon data |
| `generate-fixtures` | Write synthetic GeoJSON test data |
| `completions` | Print a shell completion script |

`tptgis <command> --help` is the authoritative flag list. [CLI usage](../docs/cli-usage.md)
has longer examples.

### `spatial-join`

```sh
tptgis spatial-join --points points.geojson --polygons zones.geojson \
    --output joined.geojson --format geojson
```

- `--format geojson` (default) or `csv`.
- `--incremental` inserts into the R-tree one entry at a time instead of bulk loading.
- Omit `--output` to write to standard output.

### `reproject`

```sh
tptgis reproject --input input.geojson --crs web-mercator --output reprojected.geojson
tptgis reproject --input input.geojson --crs utm-18n
tptgis reproject --input input.geojson --crs EPSG:3857
```

`--crs` accepts `web-mercator`, a UTM zone such as `utm-18n` or `utm-33s`, or an EPSG code
(`4326`, `3857`, `32601`–`32660`, `32701`–`32760`, with an optional `EPSG:` prefix).

### `convert`

```sh
tptgis convert --input roads.shp --output roads.gpkg
tptgis convert --input roads.shp --output roads.gpkg --spatial-index
cat roads.wkt | tptgis convert --input - --from wkt --to geojson
```

- Formats are inferred from the file extensions. `--from` and `--to` override them. Use
  `--from` when reading standard input.
- Shapefile output writes a `.prj` sidecar. Shapefile attributes carry through.
- `--spatial-index` writes the GeoPackage R-tree (GeoPackage output only).

### `info`

```sh
tptgis info cities.geojson
```

Prints JSON with the feature count, geometry types, bounds and property names. For a
shapefile it also reports the projection.

### `validate`

```sh
tptgis validate zones.geojson
tptgis validate cog.tif
```

Accepts `.geojson`, `.json`, `.wkt`, `.wkb`, `.shp`, `.gpkg`, `.tif` and `.tiff`. For
GeoTIFFs it checks the Cloud Optimized GeoTIFF layout. It prints a JSON report and exits
with an error if any problem is found.

### `geofence-compile`

```sh
tptgis geofence-compile --input no-fly.geojson --output src/zones.rs
```

Each polygon's `name` property names its zone. The output is a Rust module of `const`
bounding boxes, exterior rings and holes, ready for the `no_std` geofence check in
[`drone-geofence-demo`](../examples/drone-geofence-demo).

### `generate-fixtures`

```sh
tptgis generate-fixtures --points 10000 --polygons 500 --seed 42 --output fixtures/
```

Writes reproducible synthetic GeoJSON for benchmarks and tests.

## Tests

```sh
cargo test -p tpt-gis-cli
```

`tests/cli.rs` drives the subcommands end to end. `run_with_args` takes the arguments as a
slice and returns, so the tests do not exit the process.

## License

Licensed under either of [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0)
or [MIT license](https://opensource.org/licenses/MIT) at your option.
