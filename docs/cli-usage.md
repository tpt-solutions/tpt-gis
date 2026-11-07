# CLI usage (`tptgis`)

`tptgis` is the command-line tool in the `tpt-gis-cli` crate. Install it from source
with `cargo install --path tpt-gis-cli` (or run it in place with
`cargo run -p tpt-gis-cli --bin tptgis -- <command> …`).

Run `tptgis --help` or `tptgis <command> --help` for the authoritative flag list.

| Command | Purpose |
|---------|---------|
| [`spatial-join`](#spatial-join) | Points-in-polygons join (holes and multi-part geometries honoured) |
| [`reproject`](#reproject) | Reproject a GeoJSON file from WGS84 |
| [`convert`](#convert) | Convert between GeoJSON, WKT, WKB, Shapefile, and GeoPackage |
| [`info`](#info) | Summarise a vector file as JSON |
| [`validate`](#validate) | Check vector geometry and GeoTIFF COG layout |
| [`geofence-compile`](#geofence-compile) | Emit a `no_std` Rust module of polygon data |
| [`generate-fixtures`](#generate-fixtures) | Write synthetic GeoJSON test data |

## spatial-join

```sh
tptgis spatial-join --points <points.geojson> --polygons <polygons.geojson> \
  [--output <out>] [--format geojson|csv] [--incremental]
```

- `--points`: GeoJSON `FeatureCollection`. `Point` and `MultiPoint` features are
  joined; each member point keeps its feature's properties.
- `--polygons`: GeoJSON `FeatureCollection`. `Polygon` features keep their holes;
  `MultiPolygon` features are joined part by part under the same properties.
- `--output`: file to write. Standard output if omitted.
- `--format`: `geojson` (default) or `csv`.
- `--incremental`: insert into the R-Tree one entry at a time instead of bulk loading.

## reproject

```sh
tptgis reproject --input <input.geojson> --crs <target> [--output <out.geojson>]
```

Targets: `web-mercator` (EPSG:3857), `utm-<zone><n|s>` such as `utm-18n` or `utm-33s`,
or an EPSG code the library knows: `EPSG:4326`, `EPSG:3857`, `EPSG:32601`–`EPSG:32660`,
and `EPSG:32701`–`EPSG:32760`. Every part of multi-part geometries and geometry collections is projected.

## convert

```sh
tptgis convert --input <file|-> [--output <file>] [--from <fmt>] [--to <fmt>]
```

Formats are inferred from extensions, or set with `--from` and `--to`:

| Format | Extensions | Read | Write |
|--------|------------|------|-------|
| GeoJSON | `.geojson`, `.json` | yes | yes |
| WKT | `.wkt` | yes (one geometry per line) | yes |
| WKB | `.wkb` | yes (one geometry per file) | yes (as one `GeometryCollection`) |
| Shapefile | `.shp` | yes (geometry only) | no |
| GeoPackage | `.gpkg` | yes | yes (table `features`, geometry column `geom`, SRID 4326) |

- Use `--input -` to read standard input; it needs `--from`.
- Without `--output`, the result goes to standard output as GeoJSON unless `--to` is set.
- Shapefile attributes (`.dbf`) are not read yet, so shapefile features have no properties.

```sh
tptgis convert --input roads.shp --output roads.geojson
tptgis convert --input roads.geojson --output roads.gpkg
cat points.wkt | tptgis convert --input - --from wkt --to geojson
```

## completions

```sh
tptgis completions bash > tptgis.bash   # also zsh, fish, powershell, elvish
```

Prints a shell completion script to standard output.

## info

```sh
tptgis info <file>
```

Prints a JSON object with the format, feature count, features without geometry,
geometry-type counts, the bounding box `[min_x, min_y, max_x, max_y]`, and the
property names found across all features.

For a GeoTIFF (`.tif`, `.tiff`), it reports the image size, cell type, EPSG code,
origin and pixel size, and whether the image is tiled (with the tile size).

## validate

```sh
tptgis validate <file>
```

Checks a file and prints a JSON report with `valid`, the format, and a `problems` list.
The command exits with an error when any problem is found.

- Polygons (and parts of multi-polygons) must be valid: closed, simple rings, with holes
  inside the exterior.
- Line strings need at least two points.
- `.tif` and `.tiff` files are checked against the Cloud-Optimized GeoTIFF layout rules.
  Errors (not tiled, out-of-range data, non-internal layout) make the file invalid.
  Warnings (missing overviews, missing georeferencing) do not.

## geofence-compile

```sh
tptgis geofence-compile --input <zones.geojson> [--output <fence.rs>]
```

Compiles `Polygon` and `MultiPolygon` features into Rust source for an embedded
geofence. The output depends only on `tpt-gis-geom`, uses `'static` slices and
`const` arrays, and allocates nothing. Each zone is a `Geofence` with a `name`, a
`bbox`, an `exterior` ring, and `holes`:

```rust
mod fence;
use tpt_gis_geom::{predicates::point_in_ring, Point};

fn inside_no_fly_zone(p: Point) -> bool {
    fence::GEOFENCES.iter().any(|z| {
        z.bbox.contains_point(p)
            && point_in_ring(p, z.exterior)
            && !z.holes.iter().any(|h| point_in_ring(p, h))
    })
}
```

A feature's `name` property names its zone; otherwise zones are called `zone_<index>`.
Multi-polygon parts become `<name>_<part>`. Non-finite coordinates are rejected.

## generate-fixtures

```sh
tptgis generate-fixtures [--points <n>] [--polygons <n>] [--seed <n>] --output <dir>
```

Writes `points_<n>.geojson` and `polygons_<n>.geojson` into `<dir>`. Defaults: 10 000
points, 500 polygons, seed 42.

## Benchmarking

```sh
cargo bench -p spatial-join-cli
```

The criterion benchmark lives in `examples/spatial-join-cli` (kept out of the published
`tpt-gis-cli` crate) and measures the spatial join with bulk-load and incremental R-Tree
construction.
